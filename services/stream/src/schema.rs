//! API response types. Exported to `packages/schema` via ts-rs on `cargo test`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

macro_rules! api_type {
    ($(#[$meta:meta])* pub struct $name:ident { $($body:tt)* }) => {
        $(#[$meta])*
        #[derive(Serialize, Deserialize, TS, Debug, Clone)]
        #[ts(export, export_to = "../../../packages/schema/src/generated/")]
        pub struct $name { $($body)* }
    };
}

api_type! {
    pub struct HealthStatus {
        pub service: String,
        pub version: String,
        pub status: String,
    }
}

api_type! {
    /// The Arc network this deployment indexes.
    pub struct NetworkInfo {
        /// `mainnet` or `testnet`.
        pub key: String,
        pub name: String,
        #[ts(type = "number")]
        pub chain_id: u64,
        /// Block explorer base URL, without a trailing slash.
        pub explorer_url: String,
    }
}

api_type! {
    pub struct EntityRef {
        pub slug: String,
        pub name: String,
        pub category: String,
    }
}

api_type! {
    /// One side of a flow, with the attribution Tideline holds for it.
    pub struct Party {
        pub address: String,
        pub entity: Option<EntityRef>,
        /// Role of this address within the entity, e.g. "Gateway Wallet".
        pub label: Option<String>,
        /// protocol_registry | self_attested | curated | heuristic
        pub method: Option<String>,
        pub confidence: Option<f32>,
    }
}

api_type! {
    pub struct Flow {
        #[ts(type = "number")]
        pub id: i64,
        pub tx_hash: String,
        pub log_index: i32,
        #[ts(type = "number")]
        pub block_number: i64,
        pub block_time: String,
        pub ingested_at: String,
        /// Settlement → attributed-and-queryable, in ms. Only for flows ingested live.
        #[ts(type = "number | null")]
        pub latency_ms: Option<i64>,
        pub asset: String,
        pub kind: String,
        /// public | shielded | disclosed
        pub visibility: String,
        /// Token units as a decimal string. Null while the amount is shielded.
        pub amount: Option<String>,
        pub usd_value: Option<f64>,
        /// micro | retail | commercial | institutional
        pub size_tier: Option<String>,
        pub from: Party,
        pub to: Party,
        pub src_chain: Option<String>,
        pub dst_chain: Option<String>,
        /// full | partial | none
        pub attribution: String,
        pub narrative: String,
        /// Decoded protocol context. Licensed tier only.
        #[ts(type = "Record<string, unknown> | null")]
        pub details: Option<serde_json::Value>,
    }
}

api_type! {
    pub struct FlowPage {
        pub flows: Vec<Flow>,
        /// Pass as `before` to fetch the next page.
        #[ts(type = "number | null")]
        pub next_cursor: Option<i64>,
    }
}

api_type! {
    pub struct AssetVolume {
        pub asset: String,
        #[ts(type = "number")]
        pub flows: i64,
        pub volume_usd: f64,
        #[ts(type = "number")]
        pub shielded_flows: i64,
    }
}

api_type! {
    pub struct KindVolume {
        pub kind: String,
        #[ts(type = "number")]
        pub flows: i64,
        pub volume_usd: f64,
    }
}

api_type! {
    pub struct Corridor {
        /// in | out, relative to Arc
        pub direction: String,
        /// cctp | gateway
        pub protocol: String,
        pub domain: i32,
        pub chain: String,
        #[ts(type = "number")]
        pub flows: i64,
        pub volume_usd: f64,
    }
}

api_type! {
    pub struct EntityActivity {
        pub entity: EntityRef,
        #[ts(type = "number")]
        pub flows: i64,
        pub inflow_usd: f64,
        pub outflow_usd: f64,
    }
}

api_type! {
    pub struct TimeBucket {
        pub t: String,
        pub asset: String,
        #[ts(type = "number")]
        pub flows: i64,
        pub volume_usd: f64,
    }
}

api_type! {
    pub struct MethodCoverage {
        pub method: String,
        #[ts(type = "number")]
        pub addresses: i64,
    }
}

api_type! {
    pub struct VisibilityCount {
        pub visibility: String,
        #[ts(type = "number")]
        pub flows: i64,
        pub volume_usd: f64,
    }
}

api_type! {
    pub struct UnattributedAddress {
        pub address: String,
        #[ts(type = "number")]
        pub flows: i64,
        pub volume_usd: f64,
        /// Share of window volume this address touches: the coverage gained by labelling it.
        pub volume_share_pct: f64,
    }
}

api_type! {
    /// What the stored data can back, so a window is never read as more than it holds.
    pub struct DataScope {
        /// Block time of the earliest flow on file. Null until the first flow is stored.
        pub history_start: Option<String>,
        /// Seconds from `history_start` to now: how much history is on file.
        #[ts(type = "number | null")]
        pub history_seconds: Option<i64>,
        /// False when the requested window reaches back before `history_start`: its
        /// totals then cover only the time since `history_start`.
        pub window_complete: bool,
        /// Whether Arc confidential transfers are being ingested. False until Arc ships
        /// them; every flow is public until then.
        pub shielding_live: bool,
    }
}

api_type! {
    /// The falsifiable claim: how much flow is attributed, and how fast after settlement.
    pub struct Coverage {
        pub window_hours: i32,
        pub scope: DataScope,
        #[ts(type = "number")]
        pub flows_total: i64,
        /// At least one side resolves to a known entity.
        #[ts(type = "number")]
        pub flows_attributed: i64,
        /// Every real counterparty resolves to a known entity.
        #[ts(type = "number")]
        pub flows_fully_attributed: i64,
        pub flow_coverage_pct: f64,
        pub volume_usd_total: f64,
        pub volume_usd_attributed: f64,
        pub volume_coverage_pct: f64,
        /// Coverage counting only non-protocol attributions (self-attested, curated,
        /// heuristic): the proprietary part of the dataset.
        pub institutional_volume_coverage_pct: f64,
        #[ts(type = "number")]
        pub live_flows: i64,
        pub latency_p50_ms: Option<f64>,
        pub latency_p95_ms: Option<f64>,
        pub claim_within_seconds: i32,
        /// Share of live volume attributed within `claim_within_seconds` of settlement.
        pub claim_volume_pct: Option<f64>,
        pub methods: Vec<MethodCoverage>,
        pub visibility: Vec<VisibilityCount>,
        pub top_unattributed: Vec<UnattributedAddress>,
    }
}

api_type! {
    pub struct IngestStatus {
        pub running: bool,
        #[ts(type = "number | null")]
        pub last_block: Option<i64>,
        #[ts(type = "number | null")]
        pub chain_head: Option<i64>,
        #[ts(type = "number | null")]
        pub lag_blocks: Option<i64>,
        pub last_error: Option<String>,
    }
}

api_type! {
    pub struct ReferenceRatesInfo {
        pub eurc_usd: f64,
        pub usyc_usd: f64,
        /// Where the rate came from, e.g. "ECB euro reference rate".
        pub eurc_source: String,
        /// Publication date (YYYY-MM-DD). Null for a configured static rate.
        pub eurc_as_of: Option<String>,
        pub usyc_source: String,
        pub usyc_as_of: Option<String>,
        pub note: String,
    }
}

api_type! {
    pub struct Overview {
        pub window_hours: i32,
        pub generated_at: String,
        pub ingest: IngestStatus,
        #[ts(type = "number")]
        pub flows: i64,
        pub volume_usd: f64,
        #[ts(type = "number")]
        pub active_addresses: i64,
        pub assets: Vec<AssetVolume>,
        pub kinds: Vec<KindVolume>,
        pub corridors: Vec<Corridor>,
        pub top_entities: Vec<EntityActivity>,
        pub series: Vec<TimeBucket>,
        pub series_bucket_minutes: i32,
        pub coverage: Coverage,
        pub reference_rates: ReferenceRatesInfo,
    }
}

api_type! {
    pub struct EntitySummary {
        pub slug: String,
        pub name: String,
        pub category: String,
        pub description: String,
        pub website: Option<String>,
        pub source: String,
        #[ts(type = "number")]
        pub address_count: i64,
        /// attributed: at least one address on file. watchlist: known participant, no
        /// address attributed yet.
        pub status: String,
        #[ts(type = "number")]
        pub flows_24h: i64,
        pub volume_usd_24h: f64,
    }
}

api_type! {
    pub struct AddressLabel {
        pub address: String,
        pub entity: EntityRef,
        pub label: String,
        pub method: String,
        pub confidence: f32,
        pub source: String,
        pub created_at: String,
        /// The proof behind the attribution. Licensed tier only.
        #[ts(type = "Record<string, unknown> | null")]
        pub evidence: Option<serde_json::Value>,
    }
}

api_type! {
    pub struct Counterparty {
        pub address: String,
        pub entity: Option<EntityRef>,
        #[ts(type = "number")]
        pub flows: i64,
        pub volume_usd: f64,
    }
}

api_type! {
    pub struct EntityProfile {
        pub entity: EntitySummary,
        pub window_hours: i32,
        pub scope: DataScope,
        pub addresses: Vec<AddressLabel>,
        pub inflow_usd: f64,
        pub outflow_usd: f64,
        #[ts(type = "number")]
        pub flows: i64,
        #[ts(type = "number")]
        pub shielded_flows: i64,
        pub counterparties: Vec<Counterparty>,
        pub recent_flows: Vec<Flow>,
    }
}

api_type! {
    pub struct AddressLink {
        pub address: String,
        pub link_type: String,
        pub evidence_tx: String,
        #[ts(type = "number")]
        pub seen_count: i64,
    }
}

api_type! {
    pub struct AddressProfile {
        pub address: String,
        pub labels: Vec<AddressLabel>,
        pub links: Vec<AddressLink>,
        #[ts(type = "number")]
        pub flows: i64,
        pub inflow_usd: f64,
        pub outflow_usd: f64,
        pub recent_flows: Vec<Flow>,
    }
}

api_type! {
    /// "Show me every flow touching this entity in the window, and prove the attribution."
    pub struct ComplianceReport {
        pub entity: EntitySummary,
        pub window_hours: i32,
        pub generated_at: String,
        pub addresses: Vec<AddressLabel>,
        #[ts(type = "number")]
        pub flow_count: i64,
        #[ts(type = "number")]
        pub shielded_flow_count: i64,
        pub inflow_usd: f64,
        pub outflow_usd: f64,
        /// True when the window held more flows than `flows` carries.
        pub truncated: bool,
        pub flows: Vec<Flow>,
        pub methodology: String,
    }
}

api_type! {
    pub struct ApiKeyCreated {
        pub name: String,
        pub tier: String,
        /// Shown once. Only its hash is stored.
        pub key: String,
    }
}

api_type! {
    pub struct ApiError {
        pub error: String,
        pub message: String,
    }
}
