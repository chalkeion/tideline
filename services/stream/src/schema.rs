use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Serialize, Deserialize, TS, Debug, Clone)]
#[ts(export, export_to = "../../../packages/schema/src/generated/")]
pub struct HealthStatus {
    pub service: String,
    pub version: String,
    pub status: String,
}
