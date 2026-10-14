use std::net::SocketAddr;
use std::sync::Arc;

use arc_stablecoin_stream::api::{self, AppState};
use arc_stablecoin_stream::config::Config;
use arc_stablecoin_stream::db;
use arc_stablecoin_stream::ingest::{Ingestor, Progress};
use arc_stablecoin_stream::rates::LiveRates;
use arc_stablecoin_stream::rpc::RpcClient;
use arc_stablecoin_stream::tokens;
use tokio::sync::broadcast;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

const USAGE: &str = "usage:
  arc-stablecoin-stream                       run the API and the chain ingestor
  arc-stablecoin-stream keys create <name> [licensed|admin]
                                              mint a data-license API key";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load the repo-root .env when run from services/stream, then any local override.
    dotenvy::from_filename("../../.env").ok();
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        // stderr, so `keys create` prints nothing but the key on stdout.
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();

    let cfg = Arc::new(Config::from_env()?);
    let pool = db::connect(&cfg.database_url).await?;
    db::bind_network(&pool, cfg.network).await?;
    db::seed_registry(&pool, cfg.network).await?;
    tokens::seed(&pool, cfg.network).await?;

    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        [] | ["serve"] => serve(cfg, pool).await,
        ["keys", "create", name, rest @ ..] => {
            let tier = rest.first().copied().unwrap_or("licensed");
            let key = db::create_api_key(&pool, name, tier).await?;
            println!("{key}");
            eprintln!("Created {tier} key '{name}'. Store it now: only its hash is kept.");
            Ok(())
        }
        _ => {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    }
}

async fn serve(cfg: Arc<Config>, pool: sqlx::PgPool) -> anyhow::Result<()> {
    let progress = Arc::new(Progress::new());
    let (new_flows, _) = broadcast::channel(256);

    let rates = Arc::new(LiveRates::new(cfg.rates)?);
    if cfg.live_rates {
        // One fetch before ingest starts, so the first flows are valued at published rates.
        rates.refresh().await;
        tokio::spawn(rates.clone().run());
    } else {
        tracing::info!("LIVE_RATES=false: using the configured reference rates");
    }

    if let Some(hours) = cfg.retention_hours {
        let pool = pool.clone();
        tokio::spawn(async move {
            let mut every = tokio::time::interval(std::time::Duration::from_secs(60 * 60));
            loop {
                every.tick().await;
                match db::prune_flows(&pool, hours).await {
                    Ok(0) => {}
                    Ok(n) => tracing::info!(deleted = n, hours, "pruned flows past retention"),
                    Err(e) => tracing::warn!("pruning flows failed: {e}"),
                }
            }
        });
    }

    if cfg.ingest_enabled {
        let rpc = RpcClient::new(&cfg.rpc_urls)?;
        rpc.check_chain(cfg.network).await?;
        tracing::info!(
            "indexing {} (chain {}), {}",
            cfg.network.name(),
            cfg.network.chain_id(),
            if cfg.track_all_tokens {
                "every token"
            } else {
                "listed tokens only"
            }
        );
        let ingestor = Ingestor {
            cfg: cfg.clone(),
            pool: pool.clone(),
            rpc,
            rates: rates.clone(),
            progress: progress.clone(),
            new_flows: new_flows.clone(),
            tokens: tokens::load(&pool).await?,
        };
        tokio::spawn(ingestor.run());
    } else {
        tracing::info!("INGEST_ENABLED=false: serving the API only");
    }
    if cfg.admin_key.is_none() {
        tracing::warn!(
            "TIDELINE_ADMIN_KEY is not set: admin endpoints need a key from `keys create`"
        );
    }

    let addr = SocketAddr::from(([0, 0, 0, 0], cfg.port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("arc-stablecoin-stream listening on {addr}");
    let app = api::app(AppState::new(pool, cfg, progress, new_flows, rates));
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    Ok(())
}
