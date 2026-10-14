//! Postgres: pool, migrations, the attribution registry and the ingest write path.

use alloy_primitives::{keccak256, Address, PrimitiveSignature};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, QueryBuilder, Row};

use crate::config::Network;
use crate::enrich::{NewFlow, NewLink};

pub const ZERO_ADDRESS: &str = "0x0000000000000000000000000000000000000000";

const SEED: &str = include_str!("../data/entities.seed.json");

/// Confidence of a heuristic label relative to the attribution it was derived from.
const LINK_CONFIDENCE_FACTOR: f32 = 0.75;

pub async fn connect(database_url: &str) -> anyhow::Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(std::time::Duration::from_secs(10))
        .connect(database_url)
        .await
        .map_err(|e| {
            // sqlx says "pool timed out" when nothing is listening; name the likely cause.
            let target = database_url
                .rsplit_once('@')
                .map_or("DATABASE_URL", |(_, host)| host);
            anyhow::anyhow!("cannot reach Postgres at {target} ({e}): is the database running?")
        })?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

/// Lowercased `0x` + 40 hex, or `None` if `input` is not an EVM address.
pub fn normalize_address(input: &str) -> Option<String> {
    input
        .trim()
        .parse::<Address>()
        .ok()
        .map(|a| format!("{a:#x}"))
}

#[derive(Deserialize)]
struct SeedFile {
    entities: Vec<SeedEntity>,
}

#[derive(Deserialize)]
struct SeedEntity {
    slug: String,
    name: String,
    category: String,
    #[serde(default)]
    description: String,
    website: Option<String>,
    #[serde(default)]
    source: String,
    #[serde(default)]
    addresses: Vec<SeedAddress>,
}

#[derive(Deserialize)]
struct SeedAddress {
    /// `mainnet` or `testnet`: protocol contracts live at different addresses on each.
    network: String,
    address: String,
    label: String,
}

/// Tie the database to one Arc network on first start, and refuse any other afterwards.
/// Mixing networks would resume the cursor at another chain's block and label testnet
/// contracts as mainnet ones.
pub async fn bind_network(pool: &PgPool, network: Network) -> anyhow::Result<()> {
    let chain_id = network.chain_id() as i64;
    sqlx::query("INSERT INTO network (id, chain_id) VALUES (1, $1) ON CONFLICT (id) DO NOTHING")
        .bind(chain_id)
        .execute(pool)
        .await?;
    let bound: i64 = sqlx::query_scalar("SELECT chain_id FROM network WHERE id = 1")
        .fetch_one(pool)
        .await?;
    if bound != chain_id {
        let holds = Network::from_chain_id(bound as u64)
            .map_or_else(|| format!("chain {bound}"), |n| n.name().to_string());
        anyhow::bail!(
            "this database holds {holds} data, but ARC_NETWORK is {}: give each network its \
             own database (DATABASE_URL)",
            network.key()
        );
    }
    Ok(())
}

/// Load the built-in registry: Arc's protocol contracts on `network` (ground truth) and the
/// known institutional participants, which start with no addresses until attributed.
pub async fn seed_registry(pool: &PgPool, network: Network) -> anyhow::Result<()> {
    let seed: SeedFile = serde_json::from_str(SEED)?;
    for e in seed.entities {
        let entity_id = upsert_entity(
            pool,
            &e.slug,
            &e.name,
            &e.category,
            &e.description,
            e.website.as_deref(),
            &e.source,
        )
        .await?;
        for a in e
            .addresses
            .into_iter()
            .filter(|a| a.network == network.key())
        {
            let address = normalize_address(&a.address)
                .ok_or_else(|| anyhow::anyhow!("seed: bad address {}", a.address))?;
            insert_label(
                pool,
                &NewLabel {
                    address,
                    entity_id,
                    label: a.label,
                    method: "protocol_registry".into(),
                    confidence: 1.0,
                    evidence: json!({
                        "registry": "Arc contract addresses",
                        "url": "https://docs.arc.io/arc/references/contract-addresses",
                    }),
                    source: e.source.clone(),
                },
            )
            .await?;
        }
    }
    Ok(())
}

pub async fn upsert_entity(
    pool: &PgPool,
    slug: &str,
    name: &str,
    category: &str,
    description: &str,
    website: Option<&str>,
    source: &str,
) -> sqlx::Result<i64> {
    sqlx::query(
        "INSERT INTO entities (slug, name, category, description, website, source)
         VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (slug) DO UPDATE SET name = EXCLUDED.name, category = EXCLUDED.category,
             description = EXCLUDED.description, website = EXCLUDED.website,
             source = EXCLUDED.source
         RETURNING id",
    )
    .bind(slug)
    .bind(name)
    .bind(category)
    .bind(description)
    .bind(website)
    .bind(source)
    .fetch_one(pool)
    .await
    .map(|r| r.get(0))
}

pub async fn entity_id(pool: &PgPool, slug: &str) -> sqlx::Result<Option<i64>> {
    sqlx::query("SELECT id FROM entities WHERE slug = $1")
        .bind(slug)
        .fetch_optional(pool)
        .await
        .map(|r| r.map(|r| r.get(0)))
}

pub struct NewLabel {
    pub address: String,
    pub entity_id: i64,
    pub label: String,
    pub method: String,
    pub confidence: f32,
    pub evidence: Value,
    pub source: String,
}

/// Record an attribution claim, then extend it across observed control links.
pub async fn insert_label(pool: &PgPool, l: &NewLabel) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO address_labels (address, entity_id, label, method, confidence, evidence, source)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         ON CONFLICT (address, entity_id, method) DO UPDATE SET label = EXCLUDED.label,
             confidence = EXCLUDED.confidence, evidence = EXCLUDED.evidence,
             source = EXCLUDED.source",
    )
    .bind(&l.address)
    .bind(l.entity_id)
    .bind(&l.label)
    .bind(&l.method)
    .bind(l.confidence)
    .bind(&l.evidence)
    .bind(&l.source)
    .execute(pool)
    .await?;
    if l.method != "protocol_registry" && l.method != "heuristic" {
        propagate_links(pool).await?;
    }
    Ok(())
}

/// Clustering: an address linked on-chain to one with a first-party attribution inherits
/// that entity as a lower-confidence heuristic label, with the linking tx as evidence.
/// One hop only, and never over an address that already has a direct attribution.
pub async fn propagate_links(pool: &PgPool) -> sqlx::Result<u64> {
    let res = sqlx::query(
        "INSERT INTO address_labels (address, entity_id, label, method, confidence, evidence, source)
         SELECT pair.target, l.entity_id, 'Linked to ' || pair.anchor, 'heuristic',
                l.confidence * $1,
                jsonb_build_object('link_type', pair.link_type, 'linked_address', pair.anchor,
                                   'evidence_tx', pair.evidence_tx, 'derived_from', l.method),
                'Tideline clustering: ' || pair.link_type
         FROM (
             SELECT address_a AS anchor, address_b AS target, link_type, evidence_tx FROM address_links
             UNION ALL
             SELECT address_b, address_a, link_type, evidence_tx FROM address_links
         ) pair
         JOIN address_labels l
           ON l.address = pair.anchor AND l.method IN ('self_attested', 'curated')
         WHERE NOT EXISTS (
             SELECT 1 FROM address_labels d
             WHERE d.address = pair.target AND d.method <> 'heuristic'
         )
         ON CONFLICT (address, entity_id, method) DO NOTHING",
    )
    .bind(LINK_CONFIDENCE_FACTOR)
    .execute(pool)
    .await?;
    Ok(res.rows_affected())
}

/// The exact message an address owner signs (EIP-191) to attest control for an entity.
pub fn attestation_message(entity_slug: &str, address: &str) -> String {
    format!("Tideline attribution: address {address} is controlled by {entity_slug}.")
}

/// Verify an EIP-191 signature over [`attestation_message`] was produced by `address`.
pub fn verify_attestation(entity_slug: &str, address: &str, signature: &str) -> Result<(), String> {
    let expected = normalize_address(address).ok_or("address is not a valid EVM address")?;
    let sig: PrimitiveSignature = signature
        .trim()
        .parse()
        .map_err(|e| format!("signature is not a valid 65-byte hex signature: {e}"))?;
    let message = attestation_message(entity_slug, &expected);
    let signer = sig
        .recover_address_from_msg(message.as_bytes())
        .map_err(|e| format!("signature recovery failed: {e}"))?;
    if format!("{signer:#x}") != expected {
        return Err(format!(
            "signature was produced by {signer:#x}, not {expected}"
        ));
    }
    Ok(())
}

const FLOW_COLUMNS: usize = 17;

/// Insert flows idempotently; returns the ids of rows that were new.
pub async fn insert_flows(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    flows: &[(NewFlow, i64)],
    live_after_block: i64,
) -> sqlx::Result<Vec<i64>> {
    let mut ids = Vec::new();
    // Stay under Postgres' 65535 bind-parameter limit.
    for chunk in flows.chunks(65_000 / FLOW_COLUMNS) {
        let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(
            "INSERT INTO flows (tx_hash, log_index, block_number, block_time, live, asset,
                 asset_address, token_id, kind, from_address, to_address, amount, usd_value,
                 visibility, src_domain, dst_domain, details) ",
        );
        qb.push_values(chunk, |mut b, (f, block_time)| {
            b.push_bind(&f.tx_hash)
                .push_bind(f.log_index)
                .push_bind(f.block_number)
                .push("to_timestamp(")
                .push_bind_unseparated(*block_time as f64)
                .push_unseparated(")")
                .push_bind(f.block_number > live_after_block)
                .push_bind(&f.asset)
                .push_bind(&f.asset_address)
                .push_bind(f.token_id.map(|id| id.to_string()))
                .push_unseparated("::numeric")
                .push_bind(f.kind)
                .push_bind(&f.from_address)
                .push_bind(&f.to_address)
                .push_bind(f.amount.map(|a| a.to_string()))
                .push_unseparated("::numeric")
                .push_bind(f.usd_value)
                .push_bind(f.visibility)
                .push_bind(f.src_domain)
                .push_bind(f.dst_domain)
                .push_bind(&f.details);
        });
        qb.push(" ON CONFLICT (tx_hash, log_index) DO NOTHING RETURNING id");
        let rows = qb.build().fetch_all(&mut **tx).await?;
        ids.extend(rows.iter().map(|r| r.get::<i64, _>(0)));
    }
    Ok(ids)
}

pub async fn insert_links(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    links: &[NewLink],
) -> sqlx::Result<()> {
    for l in links {
        sqlx::query(
            "INSERT INTO address_links (address_a, address_b, link_type, evidence_tx)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (address_a, address_b, link_type)
             DO UPDATE SET seen_count = address_links.seen_count + 1",
        )
        .bind(&l.address_a)
        .bind(&l.address_b)
        .bind(l.link_type)
        .bind(&l.evidence_tx)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

pub async fn load_cursor(pool: &PgPool) -> sqlx::Result<Option<i64>> {
    sqlx::query("SELECT next_block FROM ingest_cursor WHERE id = 1")
        .fetch_optional(pool)
        .await
        .map(|r| r.map(|r| r.get(0)))
}

/// Rows deleted per statement, so pruning never holds a long lock on `flows`.
const PRUNE_BATCH: i64 = 20_000;

/// Delete flows settled more than `days` ago. Disclosures go with their flows; the
/// attribution registry and clustering links are kept. Returns the rows deleted.
pub async fn prune_flows(pool: &PgPool, days: u32) -> sqlx::Result<u64> {
    let mut deleted = 0;
    loop {
        let n = sqlx::query(
            "DELETE FROM flows WHERE id IN (
                 SELECT id FROM flows WHERE block_time < now() - make_interval(days => $1)
                 LIMIT $2)",
        )
        .bind(days as i32)
        .bind(PRUNE_BATCH)
        .execute(pool)
        .await?
        .rows_affected();
        deleted += n;
        if n < PRUNE_BATCH as u64 {
            return Ok(deleted);
        }
    }
}

pub async fn save_cursor(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    next_block: i64,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO ingest_cursor (id, next_block) VALUES (1, $1)
         ON CONFLICT (id) DO UPDATE SET next_block = EXCLUDED.next_block, updated_at = now()",
    )
    .bind(next_block)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Reveal the amount of a shielded flow. Returns false if the flow does not exist or was
/// never shielded.
pub async fn apply_disclosure(
    pool: &PgPool,
    flow_id: i64,
    amount_raw: &str,
    usd_value: Option<f64>,
    disclosed_by: &str,
    proof: &str,
) -> sqlx::Result<bool> {
    let mut tx = pool.begin().await?;
    let updated = sqlx::query(
        "UPDATE flows SET amount = $2::numeric, usd_value = $3, visibility = 'disclosed'
         WHERE id = $1 AND visibility IN ('shielded', 'disclosed')",
    )
    .bind(flow_id)
    .bind(amount_raw)
    .bind(usd_value)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if updated == 0 {
        return Ok(false);
    }
    sqlx::query(
        "INSERT INTO disclosures (flow_id, amount, disclosed_by, proof)
         VALUES ($1, $2::numeric, $3, $4)
         ON CONFLICT (flow_id) DO UPDATE SET amount = EXCLUDED.amount,
             disclosed_by = EXCLUDED.disclosed_by, proof = EXCLUDED.proof, created_at = now()",
    )
    .bind(flow_id)
    .bind(amount_raw)
    .bind(disclosed_by)
    .bind(proof)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(true)
}

pub fn hash_key(key: &str) -> String {
    format!("{:#x}", keccak256(key.as_bytes()))
}

/// Mint a data-license key. The plaintext is returned once and never stored.
pub async fn create_api_key(pool: &PgPool, name: &str, tier: &str) -> anyhow::Result<String> {
    if !matches!(tier, "licensed" | "admin") {
        anyhow::bail!("tier must be 'licensed' or 'admin'");
    }
    let secret: String = sqlx::query_scalar(
        "SELECT replace(gen_random_uuid()::text || gen_random_uuid()::text, '-', '')",
    )
    .fetch_one(pool)
    .await?;
    let key = format!("tdl_{secret}");
    sqlx::query("INSERT INTO api_keys (name, tier, key_hash, key_prefix) VALUES ($1, $2, $3, $4)")
        .bind(name)
        .bind(tier)
        .bind(hash_key(&key))
        .bind(&key[..12])
        .execute(pool)
        .await?;
    Ok(key)
}

/// `(key id, tier)` for a live key.
pub async fn lookup_api_key(pool: &PgPool, key: &str) -> sqlx::Result<Option<(i64, String)>> {
    sqlx::query("SELECT id, tier FROM api_keys WHERE key_hash = $1 AND NOT revoked")
        .bind(hash_key(key))
        .fetch_optional(pool)
        .await
        .map(|r| r.map(|r| (r.get(0), r.get(1))))
}

pub async fn record_usage(pool: &PgPool, key_id: i64) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO api_usage (key_id, day, requests) VALUES ($1, CURRENT_DATE, 1)
         ON CONFLICT (key_id, day) DO UPDATE SET requests = api_usage.requests + 1",
    )
    .bind(key_id)
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_file_parses_and_addresses_are_valid() {
        let seed: SeedFile = serde_json::from_str(SEED).unwrap();
        assert!(seed.entities.iter().any(|e| e.slug == "circle"));
        for e in &seed.entities {
            for a in &e.addresses {
                assert!(normalize_address(&a.address).is_some(), "{}", a.address);
                assert!(a.network.parse::<Network>().is_ok(), "{}", a.network);
            }
        }
    }

    #[test]
    fn seed_labels_every_decoded_contract_on_each_network() {
        let seed: SeedFile = serde_json::from_str(SEED).unwrap();
        for network in [Network::Mainnet, Network::Testnet] {
            let seeded: Vec<Address> = seed
                .entities
                .iter()
                .flat_map(|e| &e.addresses)
                .filter(|a| a.network == network.key())
                .map(|a| a.address.parse().unwrap())
                .collect();
            let c = network.contracts();
            let mut expected = c.watched();
            // The system emitter is not a contract anyone controls.
            expected.retain(|a| *a != c.native_usdc_emitter);
            expected.extend([c.cctp_token_minter, c.usyc_teller, c.fx_escrow]);
            for address in expected {
                assert!(
                    seeded.contains(&address),
                    "{} seed lacks {address}",
                    network.key()
                );
            }
        }
    }

    #[test]
    fn attestation_rejects_garbage_and_wrong_signer() {
        let addr = "0x00000000000000000000000000000000000a11ce";
        assert!(verify_attestation("acme", addr, "0x1234").is_err());
        // Well-formed signature, but not produced by `addr`.
        let sig = format!("0x{}{}1b", "11".repeat(32), "22".repeat(32));
        assert!(verify_attestation("acme", addr, &sig).is_err());
    }

    #[test]
    fn normalize_address_lowercases_and_rejects_non_addresses() {
        assert_eq!(
            normalize_address("0x89B50855Aa3bE2F677cD6303Cec089B5F319D72a").as_deref(),
            Some("0x89b50855aa3be2f677cd6303cec089b5f319d72a")
        );
        assert!(normalize_address("0x1234").is_none());
    }
}
