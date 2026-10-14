//! Minimal JSON-RPC client for the Arc node: just what the ingestor needs.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use alloy_primitives::{Address, Bytes, B256, U64};
use futures_util::future::join_all;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::config::Network;

/// A raw EVM log as returned by `eth_getLogs`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawLog {
    pub address: Address,
    pub topics: Vec<B256>,
    pub data: Bytes,
    pub block_number: U64,
    /// Arc nodes include the block timestamp on each log; absent on some providers.
    #[serde(default)]
    pub block_timestamp: Option<U64>,
    pub transaction_hash: B256,
    pub log_index: U64,
    #[serde(default)]
    pub removed: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum RpcError {
    #[error("transport: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("rpc error {code}: {message}")]
    Rpc { code: i64, message: String },
    #[error("malformed response: {0}")]
    Malformed(String),
}

/// Talks to one endpoint at a time and moves to the next when a call fails, so a provider
/// that rate-limits or bans this host does not stop ingestion.
#[derive(Clone)]
pub struct RpcClient {
    http: reqwest::Client,
    urls: Arc<[String]>,
    /// Index into `urls` of the endpoint in use, shared across clones.
    active: Arc<AtomicUsize>,
}

fn rpc_error(err: &Value) -> RpcError {
    RpcError::Rpc {
        code: err.get("code").and_then(Value::as_i64).unwrap_or(0),
        message: err
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string(),
    }
}

/// The `result` of a single JSON-RPC reply.
fn take_result(mut res: Value) -> Result<Value, RpcError> {
    if let Some(err) = res.get("error") {
        return Err(rpc_error(err));
    }
    match res.get_mut("result") {
        Some(v) => Ok(v.take()),
        None => Err(RpcError::Malformed("missing result".into())),
    }
}

fn chain_name(chain_id: u64) -> String {
    Network::from_chain_id(chain_id).map_or_else(
        || format!("chain {chain_id}"),
        |n| format!("{} (chain {chain_id})", n.name()),
    )
}

/// Host of an endpoint, for logs. Never the full URL: provider keys live in the path.
fn host(url: &str) -> String {
    reqwest::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_else(|| "unknown".into())
}

impl RpcClient {
    /// `urls` are tried in order, starting with the first.
    pub fn new(urls: &[String]) -> anyhow::Result<Self> {
        anyhow::ensure!(!urls.is_empty(), "no Arc RPC endpoint configured");
        let http = reqwest::Client::builder()
            .gzip(true)
            .timeout(Duration::from_secs(30))
            .build()?;
        Ok(Self {
            http,
            urls: urls.into(),
            active: Arc::default(),
        })
    }

    /// Move off endpoint `failed` after an error. A no-op when another call already did.
    fn fail_over(&self, failed: usize, error: &RpcError) {
        if self.urls.len() < 2 {
            return;
        }
        let next = (failed + 1) % self.urls.len();
        let moved =
            self.active
                .compare_exchange(failed, next, Ordering::Relaxed, Ordering::Relaxed);
        if moved.is_ok() {
            tracing::warn!(
                from = %host(&self.urls[failed]),
                to = %host(&self.urls[next]),
                "rpc endpoint failed ({error}); switching"
            );
        }
    }

    /// Run `request` against the endpoint in use, failing over when it errors. The caller
    /// retries, so the next attempt lands on the next endpoint.
    async fn on_active<T, F>(&self, request: impl FnOnce(usize) -> F) -> Result<T, RpcError>
    where
        F: std::future::Future<Output = Result<T, RpcError>>,
    {
        let index = self.active.load(Ordering::Relaxed);
        let result = request(index).await;
        if let Err(e) = &result {
            self.fail_over(index, e);
        }
        result
    }

    async fn post(&self, index: usize, body: &Value) -> Result<Value, RpcError> {
        let send = async {
            self.http
                .post(&self.urls[index])
                .json(body)
                .send()
                .await?
                .error_for_status()?
                .json::<Value>()
                .await
        };
        // Drop the URL from the error: it reaches the logs and the public ingest status.
        send.await.map_err(|e| RpcError::Transport(e.without_url()))
    }

    async fn call(&self, method: &str, params: Value) -> Result<Value, RpcError> {
        let body = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params });
        self.on_active(|index| async move { take_result(self.post(index, &body).await?) })
            .await
    }

    /// Ask every endpoint for its chain id. One that serves another chain is a
    /// misconfiguration and fails startup; one that does not answer is only logged, since a
    /// temporary ban must not keep the service down.
    pub async fn check_chain(&self, network: Network) -> anyhow::Result<()> {
        let body = json!({ "jsonrpc": "2.0", "id": 1, "method": "eth_chainId", "params": [] });
        let replies = join_all((0..self.urls.len()).map(|index| {
            let body = &body;
            async move {
                let id = take_result(self.post(index, body).await?)?;
                serde_json::from_value::<U64>(id)
                    .map(|id| id.to::<u64>())
                    .map_err(|e| RpcError::Malformed(e.to_string()))
            }
        }))
        .await;
        for (url, reply) in self.urls.iter().zip(replies) {
            match reply {
                Ok(id) if id == network.chain_id() => {}
                Ok(id) => anyhow::bail!(
                    "rpc endpoint {} serves {}, but ARC_NETWORK is {} (chain {})",
                    host(url),
                    chain_name(id),
                    network.key(),
                    network.chain_id()
                ),
                Err(e) => tracing::warn!(endpoint = %host(url), "could not confirm chain id: {e}"),
            }
        }
        Ok(())
    }

    /// The head and every matching log from `from` up to it, in one HTTP round-trip. Used
    /// at the tip, where a second round-trip would double settlement-to-ingest latency.
    pub async fn head_and_logs(
        &self,
        from: u64,
        addresses: &[Address],
        topics: &[B256],
    ) -> Result<(u64, Vec<RawLog>), RpcError> {
        let filter = json!({
            "fromBlock": format!("0x{from:x}"),
            "toBlock": "latest",
            "address": addresses,
            "topics": [topics],
        });
        let body = json!([
            { "jsonrpc": "2.0", "id": 1, "method": "eth_blockNumber", "params": [] },
            { "jsonrpc": "2.0", "id": 2, "method": "eth_getLogs", "params": [filter] },
        ]);
        let replies = self
            .on_active(|index| async move {
                let replies: Vec<Value> = serde_json::from_value(self.post(index, &body).await?)
                    .map_err(|e| RpcError::Malformed(format!("batch reply: {e}")))?;
                // An endpoint that answers but cannot serve the batch is as unusable as
                // one that is down, so a per-call error fails over too.
                match replies.iter().find_map(|r| r.get("error")) {
                    Some(err) => Err(rpc_error(err)),
                    None => Ok(replies),
                }
            })
            .await?;
        let reply = |id: u64| -> Result<Value, RpcError> {
            replies
                .iter()
                .find(|r| r.get("id").and_then(Value::as_u64) == Some(id))
                .and_then(|r| r.get("result").cloned())
                .ok_or_else(|| RpcError::Malformed(format!("batch reply {id} missing")))
        };
        let malformed = |e: serde_json::Error| RpcError::Malformed(e.to_string());
        let head = serde_json::from_value::<U64>(reply(1)?)
            .map_err(malformed)?
            .to::<u64>();
        let mut logs: Vec<RawLog> = serde_json::from_value(reply(2)?).map_err(malformed)?;
        // "latest" may have advanced past the head we were told about: keep the range
        // exact so the cursor and the stored flows always agree.
        logs.retain(|l| l.block_number.to::<u64>() <= head);
        Ok((head, logs))
    }

    pub async fn block_number(&self) -> Result<u64, RpcError> {
        let v = self.call("eth_blockNumber", json!([])).await?;
        let n: U64 = serde_json::from_value(v).map_err(|e| RpcError::Malformed(e.to_string()))?;
        Ok(n.to::<u64>())
    }

    /// Unix timestamp (seconds) of a block.
    pub async fn block_timestamp(&self, number: u64) -> Result<u64, RpcError> {
        let v = self
            .call(
                "eth_getBlockByNumber",
                json!([format!("0x{number:x}"), false]),
            )
            .await?;
        let ts = v
            .get("timestamp")
            .cloned()
            .ok_or_else(|| RpcError::Malformed(format!("block {number} has no timestamp")))?;
        let ts: U64 = serde_json::from_value(ts).map_err(|e| RpcError::Malformed(e.to_string()))?;
        Ok(ts.to::<u64>())
    }

    /// Logs from `addresses` whose topic0 is any of `topics`, over an inclusive block range.
    pub async fn get_logs(
        &self,
        from: u64,
        to: u64,
        addresses: &[Address],
        topics: &[B256],
    ) -> Result<Vec<RawLog>, RpcError> {
        let filter = json!({
            "fromBlock": format!("0x{from:x}"),
            "toBlock": format!("0x{to:x}"),
            "address": addresses,
            "topics": [topics],
        });
        let v = self.call("eth_getLogs", json!([filter])).await?;
        serde_json::from_value(v).map_err(|e| RpcError::Malformed(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use axum::routing::post;
    use axum::{Json, Router};

    use super::*;

    async fn serve(app: Router) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        url
    }

    #[tokio::test]
    async fn a_failing_endpoint_hands_over_to_the_next() {
        let banned = serve(Router::new().route(
            "/",
            post(|| async { (StatusCode::FORBIDDEN, "error code: 1006") }),
        ))
        .await;
        let healthy = serve(Router::new().route(
            "/",
            post(|| async { Json(json!({ "jsonrpc": "2.0", "id": 1, "result": "0x10" })) }),
        ))
        .await;
        let rpc = RpcClient::new(&[banned, healthy]).unwrap();

        let err = rpc.block_number().await.unwrap_err().to_string();
        assert!(err.contains("403"), "{err}");
        // The endpoint URL can carry a provider key, so it never reaches the error.
        assert!(!err.contains("127.0.0.1"), "{err}");

        // The retry lands on the healthy endpoint, and stays there.
        assert_eq!(rpc.block_number().await.unwrap(), 16);
        assert_eq!(rpc.block_number().await.unwrap(), 16);
    }

    fn chain(id: &'static str) -> Router {
        Router::new().route(
            "/",
            post(move || async move { Json(json!({ "jsonrpc": "2.0", "id": 1, "result": id })) }),
        )
    }

    #[tokio::test]
    async fn an_endpoint_on_another_chain_fails_the_check() {
        let mainnet = serve(chain("0x13b2")).await;
        let testnet = serve(chain("0x4cef52")).await;
        let down = "http://127.0.0.1:1".to_string();

        let ok = RpcClient::new(&[down.clone(), mainnet.clone()]).unwrap();
        ok.check_chain(Network::Mainnet).await.unwrap();

        let mixed = RpcClient::new(&[mainnet, testnet]).unwrap();
        let err = mixed
            .check_chain(Network::Mainnet)
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("Arc Testnet (chain 5042002)"), "{err}");
        assert!(!err.contains("127.0.0.1:"), "{err}");
    }

    #[tokio::test]
    async fn a_single_endpoint_is_retried_in_place() {
        let rpc = RpcClient::new(&["http://127.0.0.1:1".to_string()]).unwrap();
        assert!(rpc.block_number().await.is_err());
        assert_eq!(rpc.active.load(Ordering::Relaxed), 0);
        assert!(RpcClient::new(&[]).is_err());
    }
}
