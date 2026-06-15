use axum::{routing::get, Json, Router};
use serde_json::{json, Value};

/// Build the application router.
pub fn app() -> Router {
    Router::new().route("/health", get(health))
}

async fn health() -> Json<Value> {
    Json(json!({
        "service": "arc-stablecoin-stream",
        "version": env!("CARGO_PKG_VERSION"),
        "status": "ok",
    }))
}
