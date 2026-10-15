//! HTTP API: the licensed data product.
//!
//! Three tiers. Public (no key) sees the last 24h of labelled flows. Licensed keys add
//! attribution evidence, decoded protocol context, full history, the compliance report
//! and the live stream. Admin keys curate the registry.

use std::collections::HashMap;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::{ConnectInfo, Path, Query, Request, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::middleware::{self, Next};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use chrono::{SecondsFormat, Utc};
use futures_util::stream::{self, Stream};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::PgPool;
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::aggregate::{self, FlowFilter};
use crate::cache::ResponseCache;
use crate::config::Config;
use crate::db::{self, NewLabel};
use crate::enrich;
use crate::ingest::Progress;
use crate::rates::LiveRates;
use crate::schema::*;
use crate::tokens;

const PUBLIC_HISTORY_HOURS: i32 = 24;
/// Past this age a cached aggregate is recomputed while the caller waits, so an idle
/// service never serves data that is hours old.
const CACHE_MAX_STALE_SECONDS: u64 = 10 * 60;
const REPORT_MAX_FLOWS: i64 = 5000;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub cfg: Arc<Config>,
    pub progress: Arc<Progress>,
    pub new_flows: broadcast::Sender<Vec<i64>>,
    pub rates: Arc<LiveRates>,
    limiter: Arc<Mutex<HashMap<String, (Instant, u32)>>>,
    /// Expensive aggregates, served stale-while-revalidate (see `cache`).
    cache: Arc<ResponseCache>,
}

impl AppState {
    fn shielding_live(&self) -> bool {
        self.cfg.confidential_transfer_topic.is_some()
    }

    pub fn new(
        pool: PgPool,
        cfg: Arc<Config>,
        progress: Arc<Progress>,
        new_flows: broadcast::Sender<Vec<i64>>,
        rates: Arc<LiveRates>,
    ) -> Self {
        let cache = ResponseCache::new(
            Duration::from_secs(cfg.cache_seconds),
            Duration::from_secs(CACHE_MAX_STALE_SECONDS),
        );
        Self {
            pool,
            cfg,
            progress,
            new_flows,
            rates,
            limiter: Arc::default(),
            cache,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tier {
    Public,
    Licensed,
    Admin,
}

impl Tier {
    fn name(self) -> &'static str {
        match self {
            Tier::Public => "public",
            Tier::Licensed => "licensed",
            Tier::Admin => "admin",
        }
    }

    fn requests_per_minute(self) -> Option<u32> {
        match self {
            Tier::Public => Some(120),
            Tier::Licensed => Some(1200),
            Tier::Admin => None,
        }
    }

    fn max_page(self) -> i64 {
        if self >= Tier::Licensed {
            1000
        } else {
            50
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Access {
    pub tier: Tier,
}

impl Access {
    fn licensed(self) -> bool {
        self.tier >= Tier::Licensed
    }

    fn require(self, tier: Tier) -> Result<(), ApiErr> {
        if self.tier >= tier {
            return Ok(());
        }
        Err(ApiErr(
            if self.tier == Tier::Public {
                StatusCode::UNAUTHORIZED
            } else {
                StatusCode::FORBIDDEN
            },
            "license_required",
            format!(
                "this endpoint requires a {} API key (Authorization: Bearer <key>)",
                tier.name()
            ),
        ))
    }
}

pub struct ApiErr(StatusCode, &'static str, String);

impl std::fmt::Display for ApiErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.1, self.2)
    }
}

fn to_json<T: serde::Serialize>(value: T) -> Result<Value, ApiErr> {
    serde_json::to_value(value).map_err(|e| {
        ApiErr(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal",
            format!("serialising response: {e}"),
        )
    })
}

impl ApiErr {
    fn bad_request(message: impl Into<String>) -> Self {
        Self(StatusCode::BAD_REQUEST, "bad_request", message.into())
    }

    fn not_found(what: &str) -> Self {
        Self(
            StatusCode::NOT_FOUND,
            "not_found",
            format!("{what} not found"),
        )
    }
}

impl IntoResponse for ApiErr {
    fn into_response(self) -> Response {
        let body = ApiError {
            error: self.1.to_string(),
            message: self.2,
        };
        (self.0, Json(body)).into_response()
    }
}

impl From<sqlx::Error> for ApiErr {
    fn from(e: sqlx::Error) -> Self {
        tracing::error!("database error: {e}");
        Self(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal",
            "internal error".into(),
        )
    }
}

type ApiResult<T> = Result<Json<T>, ApiErr>;

/// Liveness only: no database, no auth.
pub fn health_router() -> Router {
    Router::new().route("/health", get(health))
}

pub fn app(state: AppState) -> Router {
    let v1 = Router::new()
        .route("/network", get(network))
        .route("/overview", get(overview))
        .route("/assets", get(list_assets))
        .route("/assets/:address", get(asset_profile))
        .route("/coverage", get(coverage))
        .route("/flows", get(list_flows))
        .route("/flows/stream", get(stream_flows))
        .route("/flows/:id", get(get_flow))
        .route("/entities", get(list_entities))
        .route("/entities/:slug", get(entity_profile))
        .route("/addresses/:address", get(address_profile))
        .route("/compliance/report", get(compliance_report))
        .route("/license", get(license))
        .route("/attributions", post(create_attribution))
        .route("/attestations", post(create_attestation))
        .route("/disclosures", post(create_disclosure))
        .route("/keys", post(create_key))
        .layer(middleware::from_fn_with_state(state.clone(), authenticate))
        .with_state(state);

    health_router()
        .nest("/v1", v1)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
}

async fn health() -> Json<HealthStatus> {
    Json(HealthStatus {
        service: "arc-stablecoin-stream".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        status: "ok".into(),
    })
}

fn presented_key(headers: &HeaderMap) -> Option<&str> {
    let bearer = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));
    bearer
        .or_else(|| headers.get("x-api-key").and_then(|v| v.to_str().ok()))
        .map(str::trim)
        .filter(|k| !k.is_empty())
}

/// Resolve the caller's tier, rate-limit, and meter licensed usage.
async fn authenticate(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, ApiErr> {
    let invalid = || {
        ApiErr(
            StatusCode::UNAUTHORIZED,
            "invalid_key",
            "unknown or revoked API key".into(),
        )
    };
    let (tier, key_id, bucket) = match presented_key(req.headers()) {
        None => {
            let ip = req
                .extensions()
                .get::<ConnectInfo<SocketAddr>>()
                .map(|c| c.0.ip().to_string())
                .unwrap_or_else(|| "unknown".into());
            (Tier::Public, None, format!("ip:{ip}"))
        }
        // Compare hashes so the check does not leak the key through timing.
        Some(key)
            if state.cfg.admin_key.as_deref().map(db::hash_key) == Some(db::hash_key(key)) =>
        {
            (Tier::Admin, None, "admin".to_string())
        }
        Some(key) => {
            let (id, tier) = db::lookup_api_key(&state.pool, key)
                .await?
                .ok_or_else(invalid)?;
            let tier = if tier == "admin" {
                Tier::Admin
            } else {
                Tier::Licensed
            };
            (tier, Some(id), format!("key:{id}"))
        }
    };

    if let Some(limit) = tier.requests_per_minute() {
        let mut limiter = state.limiter.lock().unwrap();
        let now = Instant::now();
        let window = Duration::from_secs(60);
        if limiter.len() > 10_000 {
            limiter.retain(|_, (start, _)| now.duration_since(*start) < window);
        }
        let slot = limiter.entry(bucket).or_insert((now, 0));
        if now.duration_since(slot.0) >= window {
            *slot = (now, 0);
        }
        slot.1 += 1;
        if slot.1 > limit {
            return Err(ApiErr(
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limited",
                format!(
                    "{} tier is limited to {limit} requests per minute",
                    tier.name()
                ),
            ));
        }
    }

    if let Some(id) = key_id {
        if let Err(e) = db::record_usage(&state.pool, id).await {
            tracing::warn!("usage metering failed: {e}");
        }
    }
    req.extensions_mut().insert(Access { tier });
    Ok(next.run(req).await)
}

#[derive(Deserialize)]
struct WindowQuery {
    window_hours: Option<i32>,
}

impl WindowQuery {
    fn hours(&self, access: Access) -> i32 {
        let max = if access.licensed() {
            24 * 30
        } else {
            PUBLIC_HISTORY_HOURS
        };
        self.window_hours.unwrap_or(24).clamp(1, max)
    }
}

fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

async fn network(State(s): State<AppState>) -> Json<NetworkInfo> {
    let n = s.cfg.network;
    Json(NetworkInfo {
        key: n.key().into(),
        name: n.name().into(),
        chain_id: n.chain_id(),
        explorer_url: n.explorer_url().into(),
    })
}

/// The overview is an `Overview`; it is served from the aggregate cache, with ingest
/// progress and reference rates filled in live on every response. `generated_at` says
/// when the aggregates were computed.
async fn overview(
    State(s): State<AppState>,
    Extension(access): Extension<Access>,
    Query(q): Query<WindowQuery>,
) -> ApiResult<Value> {
    let hours = q.hours(access);
    let state = s.clone();
    let mut value = s
        .cache
        .get(&format!("overview:{hours}"), move || async move {
            let parts = aggregate::overview(&state.pool, hours).await?;
            let coverage = aggregate::coverage(&state.pool, hours, state.shielding_live()).await?;
            to_json(Overview {
                window_hours: hours,
                generated_at: now(),
                ingest: state.progress.snapshot(),
                flows: parts.flows,
                volume_usd: parts.volume_usd,
                active_addresses: parts.active_addresses,
                assets: parts.assets,
                asset_count: parts.asset_count,
                kinds: parts.kinds,
                corridors: parts.corridors,
                top_entities: parts.top_entities,
                series: parts.series,
                series_bucket_minutes: parts.series_bucket_minutes,
                coverage,
                reference_rates: state.rates.info(),
            })
        })
        .await?;
    value["ingest"] = to_json(s.progress.snapshot())?;
    value["reference_rates"] = to_json(s.rates.info())?;
    Ok(Json(value))
}

/// An `AssetList`, from the aggregate cache.
async fn list_assets(
    State(s): State<AppState>,
    Extension(access): Extension<Access>,
    Query(q): Query<WindowQuery>,
) -> ApiResult<Value> {
    let hours = q.hours(access);
    let pool = s.pool.clone();
    let value = s
        .cache
        .get(&format!("assets:{hours}"), move || async move {
            to_json(aggregate::assets(&pool, hours).await?)
        })
        .await?;
    Ok(Json(value))
}

async fn asset_profile(
    State(s): State<AppState>,
    Extension(access): Extension<Access>,
    Path(address): Path<String>,
    Query(q): Query<WindowQuery>,
) -> ApiResult<AssetProfile> {
    let address = db::normalize_address(&address)
        .ok_or_else(|| ApiErr::bad_request("not a valid EVM address"))?;
    aggregate::asset_profile(&s.pool, &address, q.hours(access))
        .await?
        .map(Json)
        .ok_or_else(|| ApiErr::not_found("asset"))
}

/// A `Coverage`, from the aggregate cache.
async fn coverage(
    State(s): State<AppState>,
    Extension(access): Extension<Access>,
    Query(q): Query<WindowQuery>,
) -> ApiResult<Value> {
    let hours = q.hours(access);
    let state = s.clone();
    let value = s
        .cache
        .get(&format!("coverage:{hours}"), move || async move {
            to_json(aggregate::coverage(&state.pool, hours, state.shielding_live()).await?)
        })
        .await?;
    Ok(Json(value))
}

async fn list_flows(
    State(s): State<AppState>,
    Extension(access): Extension<Access>,
    Query(mut filter): Query<FlowFilter>,
) -> ApiResult<FlowPage> {
    if let Some(address) = &filter.address {
        // Cross-chain parties can be non-EVM bytes32 values; only normalise real addresses.
        if let Some(normal) = db::normalize_address(address) {
            filter.address = Some(normal);
        }
    }
    if !access.licensed() {
        let hours = filter.since_hours.unwrap_or(PUBLIC_HISTORY_HOURS);
        filter.since_hours = Some(hours.clamp(1, PUBLIC_HISTORY_HOURS));
    }
    let page =
        aggregate::list_flows(&s.pool, &filter, access.tier.max_page(), access.licensed()).await?;
    Ok(Json(page))
}

async fn get_flow(
    State(s): State<AppState>,
    Extension(access): Extension<Access>,
    Path(id): Path<i64>,
) -> ApiResult<Flow> {
    aggregate::get_flow(&s.pool, id, access.licensed())
        .await?
        .map(Json)
        .ok_or_else(|| ApiErr::not_found("flow"))
}

/// Server-sent events: one `flow` event per newly attributed flow.
async fn stream_flows(
    State(s): State<AppState>,
    Extension(access): Extension<Access>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiErr> {
    access.require(Tier::Licensed)?;
    let rx = s.new_flows.subscribe();
    let pool = s.pool.clone();
    let events = stream::unfold(
        (rx, pool, Vec::<Flow>::new()),
        |(mut rx, pool, mut pending)| async move {
            loop {
                if let Some(flow) = pending.pop() {
                    let event = Event::default()
                        .event("flow")
                        .id(flow.id.to_string())
                        .json_data(&flow)
                        .unwrap_or_default();
                    return Some((Ok(event), (rx, pool, pending)));
                }
                match rx.recv().await {
                    Ok(ids) => match aggregate::flows_by_ids(&pool, &ids, true).await {
                        Ok(mut flows) => {
                            flows.reverse();
                            pending = flows;
                        }
                        Err(e) => tracing::warn!("stream lookup failed: {e}"),
                    },
                    // A slow consumer skipped some batches; keep going from the newest.
                    Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(broadcast::error::RecvError::Closed) => return None,
                }
            }
        },
    );
    Ok(Sse::new(events).keep_alive(KeepAlive::default()))
}

async fn list_entities(State(s): State<AppState>) -> ApiResult<Vec<EntitySummary>> {
    Ok(Json(aggregate::list_entities(&s.pool).await?))
}

async fn entity_profile(
    State(s): State<AppState>,
    Extension(access): Extension<Access>,
    Path(slug): Path<String>,
    Query(q): Query<WindowQuery>,
) -> ApiResult<EntityProfile> {
    let hours = q.hours(access);
    let entity = aggregate::get_entity(&s.pool, &slug)
        .await?
        .ok_or_else(|| ApiErr::not_found("entity"))?;
    let totals = aggregate::entity_totals(&s.pool, &slug, hours).await?;
    let filter = FlowFilter {
        entity: Some(slug.clone()),
        since_hours: Some(hours),
        limit: Some(25),
        ..Default::default()
    };
    Ok(Json(EntityProfile {
        entity,
        window_hours: hours,
        scope: aggregate::scope(&s.pool, hours, s.shielding_live()).await?,
        addresses: aggregate::entity_labels(&s.pool, &slug, access.licensed()).await?,
        inflow_usd: totals.inflow_usd,
        outflow_usd: totals.outflow_usd,
        flows: totals.flows,
        shielded_flows: totals.shielded_flows,
        counterparties: aggregate::entity_counterparties(&s.pool, &slug, hours).await?,
        recent_flows: aggregate::list_flows(&s.pool, &filter, 25, access.licensed())
            .await?
            .flows,
    }))
}

async fn address_profile(
    State(s): State<AppState>,
    Extension(access): Extension<Access>,
    Path(address): Path<String>,
) -> ApiResult<AddressProfile> {
    let address = db::normalize_address(&address)
        .ok_or_else(|| ApiErr::bad_request("not a valid EVM address"))?;
    let (flows, inflow_usd, outflow_usd) = aggregate::address_totals(&s.pool, &address).await?;
    let filter = FlowFilter {
        address: Some(address.clone()),
        since_hours: (!access.licensed()).then_some(PUBLIC_HISTORY_HOURS),
        limit: Some(25),
        ..Default::default()
    };
    Ok(Json(AddressProfile {
        labels: aggregate::address_labels(&s.pool, &address, access.licensed()).await?,
        links: aggregate::address_links(&s.pool, &address).await?,
        flows,
        inflow_usd,
        outflow_usd,
        recent_flows: aggregate::list_flows(&s.pool, &filter, 25, access.licensed())
            .await?
            .flows,
        address,
    }))
}

#[derive(Deserialize)]
struct ReportQuery {
    entity: String,
    window_hours: Option<i32>,
    format: Option<String>,
}

const METHODOLOGY: &str = "Flows are decoded from Arc on-chain events (native and ERC-20 \
transfers, CCTP V2, Gateway) and joined to Tideline's attribution registry at query time. Each attributed address \
lists its method (protocol_registry, self_attested, curated, heuristic), confidence and \
evidence. Amount-shielded flows are counted and attributed by counterparty; their value is \
excluded from USD totals unless disclosed. USD values are notional: each flow is valued at \
the reference rate in force when it was ingested.";

fn csv_field(value: &str) -> String {
    if value.contains([',', '"', '\n']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn flows_csv(flows: &[Flow]) -> String {
    let mut out = String::from(
        "flow_id,block_time,tx_hash,log_index,asset,asset_address,asset_listed,token_id,kind,\
visibility,amount,usd_value,from_address,from_entity,from_method,from_confidence,to_address,to_entity,to_method,\
to_confidence,attribution,narrative\n",
    );
    for f in flows {
        let party = |p: &Party| {
            [
                p.address.clone(),
                p.entity
                    .as_ref()
                    .map(|e| e.name.clone())
                    .unwrap_or_default(),
                p.method.clone().unwrap_or_default(),
                p.confidence.map(|c| format!("{c:.2}")).unwrap_or_default(),
            ]
        };
        let mut fields = vec![
            f.id.to_string(),
            f.block_time.clone(),
            f.tx_hash.clone(),
            f.log_index.to_string(),
            f.asset.clone(),
            f.asset_address.clone(),
            f.asset_listed.to_string(),
            f.token_id.clone().unwrap_or_default(),
            f.kind.clone(),
            f.visibility.clone(),
            f.amount.clone().unwrap_or_default(),
            f.usd_value.map(|v| format!("{v:.2}")).unwrap_or_default(),
        ];
        fields.extend(party(&f.from));
        fields.extend(party(&f.to));
        fields.push(f.attribution.clone());
        fields.push(f.narrative.clone());
        let line: Vec<String> = fields.iter().map(|v| csv_field(v)).collect();
        out.push_str(&line.join(","));
        out.push('\n');
    }
    out
}

async fn compliance_report(
    State(s): State<AppState>,
    Extension(access): Extension<Access>,
    Query(q): Query<ReportQuery>,
) -> Result<Response, ApiErr> {
    access.require(Tier::Licensed)?;
    let hours = q.window_hours.unwrap_or(24).clamp(1, 24 * 30);
    let entity = aggregate::get_entity(&s.pool, &q.entity)
        .await?
        .ok_or_else(|| ApiErr::not_found("entity"))?;
    let totals = aggregate::entity_totals(&s.pool, &q.entity, hours).await?;
    let filter = FlowFilter {
        entity: Some(q.entity.clone()),
        since_hours: Some(hours),
        limit: Some(REPORT_MAX_FLOWS),
        ..Default::default()
    };
    let page = aggregate::list_flows(&s.pool, &filter, REPORT_MAX_FLOWS, true).await?;

    if q.format.as_deref() == Some("csv") {
        let filename = format!("tideline-{}-{hours}h.csv", entity.slug);
        return Ok((
            [
                (header::CONTENT_TYPE, "text/csv; charset=utf-8".to_string()),
                (
                    header::CONTENT_DISPOSITION,
                    format!("attachment; filename=\"{filename}\""),
                ),
            ],
            flows_csv(&page.flows),
        )
            .into_response());
    }

    Ok(Json(ComplianceReport {
        addresses: aggregate::entity_labels(&s.pool, &q.entity, true).await?,
        entity,
        window_hours: hours,
        generated_at: now(),
        flow_count: totals.flows,
        shielded_flow_count: totals.shielded_flows,
        inflow_usd: totals.inflow_usd,
        outflow_usd: totals.outflow_usd,
        truncated: page.next_cursor.is_some(),
        flows: page.flows,
        methodology: METHODOLOGY.to_string(),
    })
    .into_response())
}

async fn license(Extension(access): Extension<Access>) -> Json<Value> {
    Json(json!({
        "your_tier": access.tier.name(),
        "tiers": [
            {
                "tier": "public",
                "auth": "none",
                "rate_limit_per_minute": Tier::Public.requests_per_minute(),
                "includes": ["overview", "coverage", "entities", "flows (last 24h, 50 per page)"],
            },
            {
                "tier": "licensed",
                "auth": "Authorization: Bearer <key>",
                "rate_limit_per_minute": Tier::Licensed.requests_per_minute(),
                "includes": [
                    "attribution evidence per address",
                    "decoded protocol context per flow",
                    "30 days of history, 1000 per page",
                    "compliance report (JSON and CSV)",
                    "live flow stream (SSE)",
                ],
            },
        ],
    }))
}

#[derive(Deserialize)]
struct AttributionBody {
    entity_slug: String,
    /// Required with `category` when the entity does not exist yet.
    entity_name: Option<String>,
    category: Option<String>,
    address: String,
    #[serde(default)]
    label: String,
    confidence: Option<f32>,
    #[serde(default)]
    evidence: Value,
    #[serde(default)]
    source: String,
}

async fn resolve_entity(
    pool: &PgPool,
    slug: &str,
    name: Option<&str>,
    category: Option<&str>,
) -> Result<i64, ApiErr> {
    if let Some(id) = db::entity_id(pool, slug).await? {
        return Ok(id);
    }
    match (name, category) {
        (Some(name), Some(category)) => {
            Ok(db::upsert_entity(pool, slug, name, category, "", None, "curated").await?)
        }
        _ => Err(ApiErr::bad_request(format!(
            "entity '{slug}' does not exist; send entity_name and category to create it"
        ))),
    }
}

/// Curated attribution: an analyst asserts an address belongs to an entity, with evidence.
async fn create_attribution(
    State(s): State<AppState>,
    Extension(access): Extension<Access>,
    Json(body): Json<AttributionBody>,
) -> ApiResult<Vec<AddressLabel>> {
    access.require(Tier::Admin)?;
    let address = db::normalize_address(&body.address)
        .ok_or_else(|| ApiErr::bad_request("address is not a valid EVM address"))?;
    let confidence = body.confidence.unwrap_or(0.8);
    if !(confidence > 0.0 && confidence <= 1.0) {
        return Err(ApiErr::bad_request("confidence must be in (0, 1]"));
    }
    if body.evidence.as_object().is_none_or(|o| o.is_empty()) {
        return Err(ApiErr::bad_request(
            "evidence is required: a non-empty JSON object describing why this attribution holds",
        ));
    }
    let entity_id = resolve_entity(
        &s.pool,
        &body.entity_slug,
        body.entity_name.as_deref(),
        body.category.as_deref(),
    )
    .await?;
    db::insert_label(
        &s.pool,
        &NewLabel {
            address: address.clone(),
            entity_id,
            label: body.label,
            method: "curated".into(),
            confidence,
            evidence: body.evidence,
            source: body.source,
        },
    )
    .await?;
    Ok(Json(
        aggregate::address_labels(&s.pool, &address, true).await?,
    ))
}

#[derive(Deserialize)]
struct AttestationBody {
    entity_slug: String,
    address: String,
    /// EIP-191 signature by `address` over the message from `db::attestation_message`.
    signature: String,
    #[serde(default)]
    label: String,
}

/// Self-attestation: the entity proves control of an address by signing with it. The
/// operator vouches for who the entity is (hence admin); the signature proves the key.
async fn create_attestation(
    State(s): State<AppState>,
    Extension(access): Extension<Access>,
    Json(body): Json<AttestationBody>,
) -> ApiResult<Vec<AddressLabel>> {
    access.require(Tier::Admin)?;
    let address = db::normalize_address(&body.address)
        .ok_or_else(|| ApiErr::bad_request("address is not a valid EVM address"))?;
    let entity_id = db::entity_id(&s.pool, &body.entity_slug)
        .await?
        .ok_or_else(|| ApiErr::not_found("entity"))?;
    db::verify_attestation(&body.entity_slug, &address, &body.signature)
        .map_err(ApiErr::bad_request)?;
    db::insert_label(
        &s.pool,
        &NewLabel {
            address: address.clone(),
            entity_id,
            label: body.label,
            method: "self_attested".into(),
            confidence: 0.99,
            evidence: json!({
                "scheme": "eip191",
                "message": db::attestation_message(&body.entity_slug, &address),
                "signature": body.signature.trim(),
            }),
            source: "Signed by the address owner".into(),
        },
    )
    .await?;
    Ok(Json(
        aggregate::address_labels(&s.pool, &address, true).await?,
    ))
}

/// Decimal token amount ("1250000.5") → raw units, or `None` if malformed.
fn parse_units(amount: &str, decimals: usize) -> Option<String> {
    let (whole, frac) = amount.trim().split_once('.').unwrap_or((amount.trim(), ""));
    if whole.is_empty() && frac.is_empty() || frac.len() > decimals {
        return None;
    }
    if !whole
        .chars()
        .chain(frac.chars())
        .all(|c| c.is_ascii_digit())
    {
        return None;
    }
    let raw = format!("{whole}{frac:0<decimals$}");
    let trimmed = raw.trim_start_matches('0');
    Some(if trimmed.is_empty() { "0" } else { trimmed }.to_string())
}

#[derive(Deserialize)]
struct DisclosureBody {
    flow_id: i64,
    /// Token units as a decimal string.
    amount: String,
    /// Who revealed the amount (entity slug or free text).
    disclosed_by: String,
    /// View-key proof, stored verbatim.
    #[serde(default)]
    proof: String,
}

/// Selective disclosure: reveal the amount of a shielded flow so it re-enters volume
/// analytics. Arc has not published a verifiable view-key proof format, so the disclosure
/// is recorded as operator-asserted.
async fn create_disclosure(
    State(s): State<AppState>,
    Extension(access): Extension<Access>,
    Json(body): Json<DisclosureBody>,
) -> ApiResult<Flow> {
    access.require(Tier::Admin)?;
    let flow = aggregate::get_flow(&s.pool, body.flow_id, false)
        .await?
        .ok_or_else(|| ApiErr::not_found("flow"))?;
    let token = tokens::get(&s.pool, &flow.asset_address)
        .await?
        .ok_or_else(|| ApiErr::not_found("asset"))?;
    let decimals = token.decimals.unwrap_or(0) as usize;
    let raw = parse_units(&body.amount, decimals).ok_or_else(|| {
        ApiErr::bad_request(format!(
            "amount must be a decimal with at most {decimals} places"
        ))
    })?;
    let usd = token
        .price
        .and_then(|p| s.rates.current().price(p))
        .map(|rate| enrich::usd_value(raw.parse().unwrap_or_default(), decimals as u32, rate));
    let applied = db::apply_disclosure(
        &s.pool,
        body.flow_id,
        &raw,
        usd,
        &body.disclosed_by,
        &body.proof,
    )
    .await?;
    if !applied {
        return Err(ApiErr(
            StatusCode::CONFLICT,
            "not_shielded",
            "flow amount is public; there is nothing to disclose".into(),
        ));
    }
    aggregate::get_flow(&s.pool, body.flow_id, true)
        .await?
        .map(Json)
        .ok_or_else(|| ApiErr::not_found("flow"))
}

#[derive(Deserialize)]
struct KeyBody {
    name: String,
    tier: Option<String>,
}

async fn create_key(
    State(s): State<AppState>,
    Extension(access): Extension<Access>,
    Json(body): Json<KeyBody>,
) -> ApiResult<ApiKeyCreated> {
    access.require(Tier::Admin)?;
    let tier = body.tier.unwrap_or_else(|| "licensed".into());
    let key = db::create_api_key(&s.pool, &body.name, &tier)
        .await
        .map_err(|e| ApiErr::bad_request(e.to_string()))?;
    Ok(Json(ApiKeyCreated {
        name: body.name,
        tier,
        key,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_units_scales_and_validates() {
        assert_eq!(
            parse_units("1250000.5", 6).as_deref(),
            Some("1250000500000")
        );
        assert_eq!(parse_units("0.000001", 6).as_deref(), Some("1"));
        assert_eq!(parse_units("7", 6).as_deref(), Some("7000000"));
        assert_eq!(parse_units("0", 6).as_deref(), Some("0"));
        assert!(parse_units("1.0000001", 6).is_none());
        assert!(parse_units("-5", 6).is_none());
        assert!(parse_units("", 6).is_none());
        assert!(parse_units("1e6", 6).is_none());
    }

    #[test]
    fn csv_fields_are_quoted_when_needed() {
        assert_eq!(csv_field("plain"), "plain");
        assert_eq!(csv_field("a,b"), "\"a,b\"");
        assert_eq!(csv_field("say \"hi\""), "\"say \"\"hi\"\"\"");
    }
}
