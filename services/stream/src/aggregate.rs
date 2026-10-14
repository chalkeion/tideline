//! Read side: flows joined to the attribution graph, and the aggregates built on them.

use chrono::{DateTime, SecondsFormat, Utc};
use serde::Deserialize;
use sqlx::postgres::PgRow;
use sqlx::{PgPool, Postgres, QueryBuilder, Row};

use crate::db::ZERO_ADDRESS;
use crate::enrich::{cctp_domain_name, size_tier, Narrative};
use crate::schema::*;

/// Seconds after settlement within which the headline claim counts a flow as attributed.
pub const CLAIM_WITHIN_SECONDS: i32 = 5;

const FLOW_SELECT: &str = "
SELECT f.id, f.tx_hash, f.log_index, f.block_number, f.block_time, f.ingested_at, f.live,
       f.asset, f.kind, f.from_address, f.to_address, f.amount::text AS amount, f.usd_value,
       f.visibility, f.src_domain, f.dst_domain, f.details,
       fl.slug AS from_slug, fl.name AS from_name, fl.category AS from_category,
       fl.label AS from_label, fl.method AS from_method, fl.confidence AS from_confidence,
       tl.slug AS to_slug, tl.name AS to_name, tl.category AS to_category,
       tl.label AS to_label, tl.method AS to_method, tl.confidence AS to_confidence
FROM flows f
LEFT JOIN address_best_label fl ON fl.address = f.from_address
LEFT JOIN address_best_label tl ON tl.address = f.to_address";

/// Shared FROM clause for aggregates that need both sides' attribution.
const FLOWS_LABELLED: &str = "
FROM flows f
LEFT JOIN address_best_label fl ON fl.address = f.from_address
LEFT JOIN address_best_label tl ON tl.address = f.to_address";

fn ts(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(SecondsFormat::Millis, true)
}

/// Raw token units → decimal string, trimming trailing zeros ("1020000" → "1.02").
pub fn format_units(raw: &str, decimals: usize) -> String {
    let padded = format!("{raw:0>width$}", width = decimals + 1);
    let (whole, frac) = padded.split_at(padded.len() - decimals);
    let frac = frac.trim_end_matches('0');
    if frac.is_empty() {
        whole.to_string()
    } else {
        format!("{whole}.{frac}")
    }
}

fn party(row: &PgRow, side: &str, address: String) -> Party {
    let col = |name: &str| format!("{side}_{name}");
    let slug: Option<String> = row.get(col("slug").as_str());
    Party {
        address,
        entity: slug.map(|slug| EntityRef {
            slug,
            name: row.get(col("name").as_str()),
            category: row.get(col("category").as_str()),
        }),
        label: row.get(col("label").as_str()),
        method: row.get(col("method").as_str()),
        confidence: row.get(col("confidence").as_str()),
    }
}

fn flow_from_row(row: &PgRow, with_details: bool) -> Flow {
    let block_time: DateTime<Utc> = row.get("block_time");
    let ingested_at: DateTime<Utc> = row.get("ingested_at");
    let live: bool = row.get("live");
    let from = party(row, "from", row.get("from_address"));
    let to = party(row, "to", row.get("to_address"));
    let kind: String = row.get("kind");
    let asset: String = row.get("asset");
    let visibility: String = row.get("visibility");
    let usd_value: Option<f64> = row.get("usd_value");
    let src_domain: Option<i32> = row.get("src_domain");
    let dst_domain: Option<i32> = row.get("dst_domain");
    let details: serde_json::Value = row.get("details");

    // The zero address is not a counterparty: a mint to a known entity is fully attributed.
    let known = |p: &Party| p.entity.is_some();
    let real = |p: &Party| p.address != ZERO_ADDRESS;
    let attribution = match (known(&from), known(&to)) {
        (true, true) => "full",
        (true, false) if !real(&to) => "full",
        (false, true) if !real(&from) => "full",
        (false, false) => "none",
        _ => "partial",
    };

    let narrative = Narrative {
        kind: &kind,
        asset: &asset,
        usd_value,
        visibility: &visibility,
        from_address: &from.address,
        to_address: &to.address,
        from_entity: from.entity.as_ref().map(|e| e.name.as_str()),
        to_entity: to.entity.as_ref().map(|e| e.name.as_str()),
        src_domain,
        dst_domain,
        self_bridge: details["self_bridge"].as_bool().unwrap_or(false),
    }
    .render();

    Flow {
        id: row.get("id"),
        tx_hash: row.get("tx_hash"),
        log_index: row.get("log_index"),
        block_number: row.get("block_number"),
        block_time: ts(block_time),
        ingested_at: ts(ingested_at),
        latency_ms: live.then(|| (ingested_at - block_time).num_milliseconds().max(0)),
        amount: row
            .get::<Option<String>, _>("amount")
            .map(|raw| format_units(&raw, 6)),
        size_tier: usd_value.map(|v| size_tier(v).to_string()),
        usd_value,
        src_chain: src_domain.map(cctp_domain_name),
        dst_chain: dst_domain.map(cctp_domain_name),
        attribution: attribution.to_string(),
        narrative,
        details: with_details.then_some(details),
        asset,
        kind,
        visibility,
        from,
        to,
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct FlowFilter {
    pub asset: Option<String>,
    pub kind: Option<String>,
    /// Entity slug on either side.
    pub entity: Option<String>,
    /// Address on either side.
    pub address: Option<String>,
    pub visibility: Option<String>,
    pub min_usd: Option<f64>,
    pub since_hours: Option<i32>,
    /// true: at least one side attributed. false: neither side attributed.
    pub attributed: Option<bool>,
    /// Only flows with id below this (pagination cursor).
    pub before: Option<i64>,
    /// Only flows with id above this (live tailing).
    pub after: Option<i64>,
    pub limit: Option<i64>,
}

fn push_filters(qb: &mut QueryBuilder<Postgres>, f: &FlowFilter) {
    qb.push(" WHERE true");
    if let Some(v) = &f.asset {
        qb.push(" AND f.asset = ").push_bind(v.to_uppercase());
    }
    if let Some(v) = &f.kind {
        qb.push(" AND f.kind = ").push_bind(v.clone());
    }
    if let Some(v) = &f.entity {
        qb.push(" AND (fl.slug = ")
            .push_bind(v.clone())
            .push(" OR tl.slug = ")
            .push_bind(v.clone())
            .push(")");
    }
    if let Some(v) = &f.address {
        let v = v.to_lowercase();
        qb.push(" AND (f.from_address = ")
            .push_bind(v.clone())
            .push(" OR f.to_address = ")
            .push_bind(v)
            .push(")");
    }
    if let Some(v) = &f.visibility {
        qb.push(" AND f.visibility = ").push_bind(v.clone());
    }
    if let Some(v) = f.min_usd {
        qb.push(" AND f.usd_value >= ").push_bind(v);
    }
    if let Some(v) = f.since_hours {
        qb.push(" AND f.block_time > now() - make_interval(hours => ")
            .push_bind(v)
            .push(")");
    }
    match f.attributed {
        Some(true) => qb.push(" AND (fl.address IS NOT NULL OR tl.address IS NOT NULL)"),
        Some(false) => qb.push(" AND fl.address IS NULL AND tl.address IS NULL"),
        None => qb,
    };
    if let Some(v) = f.before {
        qb.push(" AND f.id < ").push_bind(v);
    }
    if let Some(v) = f.after {
        qb.push(" AND f.id > ").push_bind(v);
    }
}

pub async fn list_flows(
    pool: &PgPool,
    filter: &FlowFilter,
    max_limit: i64,
    with_details: bool,
) -> sqlx::Result<FlowPage> {
    let limit = filter.limit.unwrap_or(50).clamp(1, max_limit);
    let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(FLOW_SELECT);
    push_filters(&mut qb, filter);
    qb.push(" ORDER BY f.id DESC LIMIT ").push_bind(limit + 1);
    let rows = qb.build().fetch_all(pool).await?;
    let mut flows: Vec<Flow> = rows
        .iter()
        .map(|r| flow_from_row(r, with_details))
        .collect();
    let next_cursor = if flows.len() as i64 > limit {
        flows.truncate(limit as usize);
        flows.last().map(|f| f.id)
    } else {
        None
    };
    Ok(FlowPage { flows, next_cursor })
}

pub async fn get_flow(pool: &PgPool, id: i64, with_details: bool) -> sqlx::Result<Option<Flow>> {
    let row = sqlx::query(&format!("{FLOW_SELECT} WHERE f.id = $1"))
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| flow_from_row(&r, with_details)))
}

pub async fn flows_by_ids(
    pool: &PgPool,
    ids: &[i64],
    with_details: bool,
) -> sqlx::Result<Vec<Flow>> {
    let rows = sqlx::query(&format!("{FLOW_SELECT} WHERE f.id = ANY($1) ORDER BY f.id"))
        .bind(ids)
        .fetch_all(pool)
        .await?;
    Ok(rows
        .iter()
        .map(|r| flow_from_row(r, with_details))
        .collect())
}

fn pct(part: f64, whole: f64) -> f64 {
    if whole > 0.0 {
        (part / whole * 1000.0).round() / 10.0
    } else {
        0.0
    }
}

/// What the stored flows can back for a window. `shielding_live` is configuration, not
/// data: whether confidential transfers are ingested at all.
pub async fn scope(
    pool: &PgPool,
    window_hours: i32,
    shielding_live: bool,
) -> sqlx::Result<DataScope> {
    let row = sqlx::query(
        "SELECT min(block_time) AS history_start,
                EXTRACT(EPOCH FROM now() - min(block_time))::bigint AS history_seconds,
                COALESCE(min(block_time) <= now() - make_interval(hours => $1), false) AS complete
         FROM flows",
    )
    .bind(window_hours)
    .fetch_one(pool)
    .await?;
    let history_start: Option<DateTime<Utc>> = row.get("history_start");
    Ok(DataScope {
        history_start: history_start.map(ts),
        history_seconds: row.get("history_seconds"),
        window_complete: row.get("complete"),
        shielding_live,
    })
}

pub async fn coverage(
    pool: &PgPool,
    window_hours: i32,
    shielding_live: bool,
) -> sqlx::Result<Coverage> {
    // A side "counts" when it is labelled; the zero address is never a real counterparty.
    let row = sqlx::query(&format!(
        "WITH w AS (
             SELECT f.usd_value, f.live, f.block_time, f.ingested_at,
                    (fl.address IS NOT NULL OR tl.address IS NOT NULL) AS any_side,
                    (fl.address IS NOT NULL OR f.from_address = $2)
                        AND (tl.address IS NOT NULL OR f.to_address = $2)
                        AND (fl.address IS NOT NULL OR tl.address IS NOT NULL) AS all_sides,
                    (COALESCE(fl.method, 'protocol_registry') <> 'protocol_registry'
                        OR COALESCE(tl.method, 'protocol_registry') <> 'protocol_registry') AS proprietary,
                    GREATEST(EXTRACT(EPOCH FROM f.ingested_at - f.block_time), 0) * 1000 AS latency_ms
             {FLOWS_LABELLED}
             WHERE f.block_time > now() - make_interval(hours => $1)
         )
         SELECT count(*) AS total,
                count(*) FILTER (WHERE any_side) AS attributed,
                count(*) FILTER (WHERE all_sides) AS fully,
                COALESCE(sum(usd_value), 0) AS volume,
                COALESCE(sum(usd_value) FILTER (WHERE any_side), 0) AS volume_attributed,
                COALESCE(sum(usd_value) FILTER (WHERE proprietary), 0) AS volume_proprietary,
                count(*) FILTER (WHERE live) AS live_flows,
                percentile_cont(0.5) WITHIN GROUP (ORDER BY latency_ms) FILTER (WHERE live) AS p50,
                percentile_cont(0.95) WITHIN GROUP (ORDER BY latency_ms) FILTER (WHERE live) AS p95,
                COALESCE(sum(usd_value) FILTER (WHERE live), 0) AS live_volume,
                COALESCE(sum(usd_value) FILTER (WHERE live AND any_side AND latency_ms <= $3 * 1000), 0)
                    AS claim_volume
         FROM w"
    ))
    .bind(window_hours)
    .bind(ZERO_ADDRESS)
    .bind(CLAIM_WITHIN_SECONDS)
    .fetch_one(pool)
    .await?;

    let total: i64 = row.get("total");
    let attributed: i64 = row.get("attributed");
    let volume: f64 = row.get("volume");
    let volume_attributed: f64 = row.get("volume_attributed");
    let live_flows: i64 = row.get("live_flows");
    let live_volume: f64 = row.get("live_volume");

    let methods = sqlx::query(
        "SELECT method, count(DISTINCT address) AS addresses FROM address_labels
         GROUP BY method ORDER BY addresses DESC",
    )
    .fetch_all(pool)
    .await?
    .iter()
    .map(|r| MethodCoverage {
        method: r.get("method"),
        addresses: r.get("addresses"),
    })
    .collect();

    let visibility = sqlx::query(
        "SELECT visibility, count(*) AS flows, COALESCE(sum(usd_value), 0) AS volume
         FROM flows WHERE block_time > now() - make_interval(hours => $1)
         GROUP BY visibility ORDER BY flows DESC",
    )
    .bind(window_hours)
    .fetch_all(pool)
    .await?
    .iter()
    .map(|r| VisibilityCount {
        visibility: r.get("visibility"),
        flows: r.get("flows"),
        volume_usd: r.get("volume"),
    })
    .collect();

    let top_unattributed = sqlx::query(
        "SELECT a.address, count(*) AS flows, COALESCE(sum(a.usd_value), 0) AS volume
         FROM (
             SELECT from_address AS address, usd_value FROM flows
             WHERE block_time > now() - make_interval(hours => $1)
             UNION ALL
             SELECT to_address, usd_value FROM flows
             WHERE block_time > now() - make_interval(hours => $1)
         ) a
         LEFT JOIN address_best_label l ON l.address = a.address
         WHERE l.address IS NULL AND a.address <> $2
         GROUP BY a.address ORDER BY volume DESC, flows DESC LIMIT 12",
    )
    .bind(window_hours)
    .bind(ZERO_ADDRESS)
    .fetch_all(pool)
    .await?
    .iter()
    .map(|r| {
        let v: f64 = r.get("volume");
        UnattributedAddress {
            address: r.get("address"),
            flows: r.get("flows"),
            volume_usd: v,
            volume_share_pct: pct(v, volume),
        }
    })
    .collect();

    Ok(Coverage {
        window_hours,
        scope: scope(pool, window_hours, shielding_live).await?,
        flows_total: total,
        flows_attributed: attributed,
        flows_fully_attributed: row.get("fully"),
        flow_coverage_pct: pct(attributed as f64, total as f64),
        volume_usd_total: volume,
        volume_usd_attributed: volume_attributed,
        volume_coverage_pct: pct(volume_attributed, volume),
        institutional_volume_coverage_pct: pct(row.get("volume_proprietary"), volume),
        live_flows,
        latency_p50_ms: row.get("p50"),
        latency_p95_ms: row.get("p95"),
        claim_within_seconds: CLAIM_WITHIN_SECONDS,
        claim_volume_pct: (live_flows > 0).then(|| pct(row.get("claim_volume"), live_volume)),
        methods,
        visibility,
        top_unattributed,
    })
}

/// Bucket width that keeps a window's series readable.
fn bucket_minutes(window_hours: i32) -> i32 {
    match window_hours {
        ..=2 => 5,
        3..=12 => 30,
        13..=72 => 60,
        _ => 24 * 60,
    }
}

pub struct OverviewParts {
    pub flows: i64,
    pub volume_usd: f64,
    pub active_addresses: i64,
    pub assets: Vec<AssetVolume>,
    pub kinds: Vec<KindVolume>,
    pub corridors: Vec<Corridor>,
    pub top_entities: Vec<EntityActivity>,
    pub series: Vec<TimeBucket>,
    pub series_bucket_minutes: i32,
}

pub async fn overview(pool: &PgPool, window_hours: i32) -> sqlx::Result<OverviewParts> {
    const WINDOW: &str = "block_time > now() - make_interval(hours => $1)";

    let totals = sqlx::query(&format!(
        "SELECT count(*) AS flows, COALESCE(sum(usd_value), 0) AS volume,
                (SELECT count(*) FROM (
                     SELECT from_address FROM flows WHERE {WINDOW}
                     UNION SELECT to_address FROM flows WHERE {WINDOW}) a) AS addresses
         FROM flows WHERE {WINDOW}"
    ))
    .bind(window_hours)
    .fetch_one(pool)
    .await?;

    let assets = sqlx::query(&format!(
        "SELECT asset, count(*) AS flows, COALESCE(sum(usd_value), 0) AS volume,
                count(*) FILTER (WHERE visibility = 'shielded') AS shielded
         FROM flows WHERE {WINDOW} GROUP BY asset ORDER BY volume DESC"
    ))
    .bind(window_hours)
    .fetch_all(pool)
    .await?
    .iter()
    .map(|r| AssetVolume {
        asset: r.get("asset"),
        flows: r.get("flows"),
        volume_usd: r.get("volume"),
        shielded_flows: r.get("shielded"),
    })
    .collect();

    let kinds = sqlx::query(&format!(
        "SELECT kind, count(*) AS flows, COALESCE(sum(usd_value), 0) AS volume
         FROM flows WHERE {WINDOW} GROUP BY kind ORDER BY volume DESC"
    ))
    .bind(window_hours)
    .fetch_all(pool)
    .await?
    .iter()
    .map(|r| KindVolume {
        kind: r.get("kind"),
        flows: r.get("flows"),
        volume_usd: r.get("volume"),
    })
    .collect();

    let corridors = sqlx::query(&format!(
        "SELECT kind, CASE WHEN kind LIKE '%outbound' THEN dst_domain ELSE src_domain END AS domain,
                count(*) AS flows, COALESCE(sum(usd_value), 0) AS volume
         FROM flows
         WHERE {WINDOW} AND kind IN ('cctp_outbound', 'cctp_inbound', 'gateway_outbound', 'gateway_inbound')
         GROUP BY 1, 2 HAVING CASE WHEN kind LIKE '%outbound' THEN dst_domain ELSE src_domain END IS NOT NULL
         ORDER BY volume DESC"
    ))
    .bind(window_hours)
    .fetch_all(pool)
    .await?
    .iter()
    .map(|r| {
        let kind: String = r.get("kind");
        let domain: i32 = r.get("domain");
        let (protocol, direction) = kind.split_once('_').unwrap_or(("cctp", "outbound"));
        Corridor {
            direction: if direction == "outbound" { "out" } else { "in" }.to_string(),
            protocol: protocol.to_string(),
            domain,
            chain: cctp_domain_name(domain),
            flows: r.get("flows"),
            volume_usd: r.get("volume"),
        }
    })
    .collect();

    let top_entities = sqlx::query(&format!(
        "SELECT slug, name, category, count(*) AS flows,
                COALESCE(sum(usd_value) FILTER (WHERE side = 'in'), 0) AS inflow,
                COALESCE(sum(usd_value) FILTER (WHERE side = 'out'), 0) AS outflow
         FROM (
             SELECT l.slug, l.name, l.category, 'out' AS side, f.usd_value
             FROM flows f JOIN address_best_label l ON l.address = f.from_address WHERE f.{WINDOW}
             UNION ALL
             SELECT l.slug, l.name, l.category, 'in', f.usd_value
             FROM flows f JOIN address_best_label l ON l.address = f.to_address WHERE f.{WINDOW}
         ) x
         GROUP BY slug, name, category
         ORDER BY COALESCE(sum(usd_value), 0) DESC, flows DESC LIMIT 10"
    ))
    .bind(window_hours)
    .fetch_all(pool)
    .await?
    .iter()
    .map(|r| EntityActivity {
        entity: EntityRef {
            slug: r.get("slug"),
            name: r.get("name"),
            category: r.get("category"),
        },
        flows: r.get("flows"),
        inflow_usd: r.get("inflow"),
        outflow_usd: r.get("outflow"),
    })
    .collect();

    let minutes = bucket_minutes(window_hours);
    let series = sqlx::query(&format!(
        "SELECT date_bin(make_interval(mins => $2), block_time, TIMESTAMPTZ 'epoch') AS t, asset,
                count(*) AS flows, COALESCE(sum(usd_value), 0) AS volume
         FROM flows WHERE {WINDOW} GROUP BY 1, 2 ORDER BY 1, 2"
    ))
    .bind(window_hours)
    .bind(minutes)
    .fetch_all(pool)
    .await?
    .iter()
    .map(|r| TimeBucket {
        t: ts(r.get("t")),
        asset: r.get("asset"),
        flows: r.get("flows"),
        volume_usd: r.get("volume"),
    })
    .collect();

    Ok(OverviewParts {
        flows: totals.get("flows"),
        volume_usd: totals.get("volume"),
        active_addresses: totals.get("addresses"),
        assets,
        kinds,
        corridors,
        top_entities,
        series,
        series_bucket_minutes: minutes,
    })
}

const ENTITY_SELECT: &str = "
SELECT e.slug, e.name, e.category, e.description, e.website, e.source,
       (SELECT count(DISTINCT address) FROM address_labels l WHERE l.entity_id = e.id) AS address_count,
       COALESCE(a.flows, 0) AS flows_24h, COALESCE(a.volume, 0) AS volume_24h
FROM entities e
LEFT JOIN (
    SELECT slug, count(*) AS flows, COALESCE(sum(usd_value), 0) AS volume
    FROM (
        SELECT DISTINCT f.id, l.slug, f.usd_value
        FROM flows f
        JOIN address_best_label l ON l.address IN (f.from_address, f.to_address)
        WHERE f.block_time > now() - interval '24 hours'
    ) touched
    GROUP BY slug
) a ON a.slug = e.slug";

fn entity_from_row(r: &PgRow) -> EntitySummary {
    let address_count: i64 = r.get("address_count");
    EntitySummary {
        slug: r.get("slug"),
        name: r.get("name"),
        category: r.get("category"),
        description: r.get("description"),
        website: r.get("website"),
        source: r.get("source"),
        address_count,
        status: if address_count > 0 {
            "attributed"
        } else {
            "watchlist"
        }
        .to_string(),
        flows_24h: r.get("flows_24h"),
        volume_usd_24h: r.get("volume_24h"),
    }
}

pub async fn list_entities(pool: &PgPool) -> sqlx::Result<Vec<EntitySummary>> {
    let rows = sqlx::query(&format!(
        "SELECT * FROM ({ENTITY_SELECT}) q
         ORDER BY (address_count > 0) DESC, volume_24h DESC, name"
    ))
    .fetch_all(pool)
    .await?;
    Ok(rows.iter().map(entity_from_row).collect())
}

pub async fn get_entity(pool: &PgPool, slug: &str) -> sqlx::Result<Option<EntitySummary>> {
    let row = sqlx::query(&format!("{ENTITY_SELECT} WHERE e.slug = $1"))
        .bind(slug)
        .fetch_optional(pool)
        .await?;
    Ok(row.as_ref().map(entity_from_row))
}

const LABEL_SELECT: &str = "
SELECT l.address, l.label, l.method, l.confidence, l.source, l.created_at, l.evidence,
       e.slug, e.name, e.category
FROM address_labels l JOIN entities e ON e.id = l.entity_id";

fn label_from_row(r: &PgRow, with_evidence: bool) -> AddressLabel {
    AddressLabel {
        address: r.get("address"),
        entity: EntityRef {
            slug: r.get("slug"),
            name: r.get("name"),
            category: r.get("category"),
        },
        label: r.get("label"),
        method: r.get("method"),
        confidence: r.get("confidence"),
        source: r.get("source"),
        created_at: ts(r.get("created_at")),
        evidence: with_evidence.then(|| r.get("evidence")),
    }
}

pub async fn entity_labels(
    pool: &PgPool,
    slug: &str,
    with_evidence: bool,
) -> sqlx::Result<Vec<AddressLabel>> {
    let rows = sqlx::query(&format!(
        "{LABEL_SELECT} WHERE e.slug = $1 ORDER BY l.confidence DESC, l.id"
    ))
    .bind(slug)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .iter()
        .map(|r| label_from_row(r, with_evidence))
        .collect())
}

pub async fn address_labels(
    pool: &PgPool,
    address: &str,
    with_evidence: bool,
) -> sqlx::Result<Vec<AddressLabel>> {
    let rows = sqlx::query(&format!(
        "{LABEL_SELECT} WHERE l.address = $1 ORDER BY l.confidence DESC, l.id"
    ))
    .bind(address)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .iter()
        .map(|r| label_from_row(r, with_evidence))
        .collect())
}

pub struct EntityTotals {
    pub flows: i64,
    pub shielded_flows: i64,
    pub inflow_usd: f64,
    pub outflow_usd: f64,
}

/// Window totals for an entity. A flow between two of its own addresses counts once, as
/// both inflow and outflow.
pub async fn entity_totals(
    pool: &PgPool,
    slug: &str,
    window_hours: i32,
) -> sqlx::Result<EntityTotals> {
    let r = sqlx::query(&format!(
        "SELECT count(*) AS flows,
                count(*) FILTER (WHERE f.visibility = 'shielded') AS shielded,
                COALESCE(sum(f.usd_value) FILTER (WHERE tl.slug = $1), 0) AS inflow,
                COALESCE(sum(f.usd_value) FILTER (WHERE fl.slug = $1), 0) AS outflow
         {FLOWS_LABELLED}
         WHERE (fl.slug = $1 OR tl.slug = $1)
           AND f.block_time > now() - make_interval(hours => $2)"
    ))
    .bind(slug)
    .bind(window_hours)
    .fetch_one(pool)
    .await?;
    Ok(EntityTotals {
        flows: r.get("flows"),
        shielded_flows: r.get("shielded"),
        inflow_usd: r.get("inflow"),
        outflow_usd: r.get("outflow"),
    })
}

pub async fn entity_counterparties(
    pool: &PgPool,
    slug: &str,
    window_hours: i32,
) -> sqlx::Result<Vec<Counterparty>> {
    let rows = sqlx::query(&format!(
        "SELECT x.address, l.slug, l.name, l.category, count(*) AS flows,
                COALESCE(sum(x.usd_value), 0) AS volume
         FROM (
             SELECT CASE WHEN fl.slug = $1 THEN f.to_address ELSE f.from_address END AS address,
                    f.usd_value
             {FLOWS_LABELLED}
             WHERE (fl.slug = $1 OR tl.slug = $1)
               AND f.block_time > now() - make_interval(hours => $2)
         ) x
         LEFT JOIN address_best_label l ON l.address = x.address
         WHERE x.address <> $3
         GROUP BY x.address, l.slug, l.name, l.category
         ORDER BY volume DESC, flows DESC LIMIT 12"
    ))
    .bind(slug)
    .bind(window_hours)
    .bind(ZERO_ADDRESS)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .iter()
        .map(|r| {
            let slug: Option<String> = r.get("slug");
            Counterparty {
                address: r.get("address"),
                entity: slug.map(|slug| EntityRef {
                    slug,
                    name: r.get("name"),
                    category: r.get("category"),
                }),
                flows: r.get("flows"),
                volume_usd: r.get("volume"),
            }
        })
        .collect())
}

pub async fn address_links(pool: &PgPool, address: &str) -> sqlx::Result<Vec<AddressLink>> {
    let rows = sqlx::query(
        "SELECT CASE WHEN address_a = $1 THEN address_b ELSE address_a END AS address,
                link_type, evidence_tx, seen_count
         FROM address_links WHERE address_a = $1 OR address_b = $1
         ORDER BY seen_count DESC",
    )
    .bind(address)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .iter()
        .map(|r| AddressLink {
            address: r.get("address"),
            link_type: r.get("link_type"),
            evidence_tx: r.get("evidence_tx"),
            seen_count: r.get("seen_count"),
        })
        .collect())
}

/// `(flows, inflow_usd, outflow_usd)` for an address over all ingested history.
pub async fn address_totals(pool: &PgPool, address: &str) -> sqlx::Result<(i64, f64, f64)> {
    let r = sqlx::query(
        "SELECT count(*) AS flows,
                COALESCE(sum(usd_value) FILTER (WHERE to_address = $1), 0) AS inflow,
                COALESCE(sum(usd_value) FILTER (WHERE from_address = $1), 0) AS outflow
         FROM flows WHERE from_address = $1 OR to_address = $1",
    )
    .bind(address)
    .fetch_one(pool)
    .await?;
    Ok((r.get("flows"), r.get("inflow"), r.get("outflow")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_units_handles_fractions_and_whole_numbers() {
        assert_eq!(format_units("1020000", 6), "1.02");
        assert_eq!(format_units("5000000", 6), "5");
        assert_eq!(format_units("1", 6), "0.000001");
        assert_eq!(format_units("0", 6), "0");
        assert_eq!(format_units("12000000000000", 6), "12000000");
    }

    #[test]
    fn pct_is_zero_on_empty_denominator() {
        assert_eq!(pct(5.0, 0.0), 0.0);
        assert_eq!(pct(1.0, 3.0), 33.3);
    }
}
