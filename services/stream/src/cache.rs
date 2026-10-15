//! Stale-while-revalidate cache for the expensive aggregate responses.
//!
//! Overview, coverage and asset aggregates scan every flow in a window, which takes seconds
//! on a small database. Once a value exists, a request is answered from memory; when the
//! value is older than the TTL, the stale copy is returned and one background task
//! recomputes it. Only a value older than `max_stale` (an idle service) is recomputed
//! while the caller waits.

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::Value;

struct Entry {
    value: Value,
    computed: Instant,
    refreshing: bool,
}

pub struct ResponseCache {
    ttl: Duration,
    max_stale: Duration,
    entries: Mutex<HashMap<String, Entry>>,
}

impl ResponseCache {
    /// A zero `ttl` disables caching: every call computes.
    pub fn new(ttl: Duration, max_stale: Duration) -> Arc<Self> {
        Arc::new(Self {
            ttl,
            max_stale,
            entries: Mutex::default(),
        })
    }

    /// The cached value for `key`, computing it with `compute` when there is none (or it
    /// is too old to serve) and refreshing it in the background when it is merely stale.
    pub async fn get<E, F, Fut>(self: &Arc<Self>, key: &str, compute: F) -> Result<Value, E>
    where
        E: std::fmt::Display + Send + 'static,
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Result<Value, E>> + Send + 'static,
    {
        if self.ttl.is_zero() {
            return compute().await;
        }
        {
            let mut entries = self.entries.lock().unwrap();
            if let Some(entry) = entries.get_mut(key) {
                let age = entry.computed.elapsed();
                if age < self.ttl {
                    return Ok(entry.value.clone());
                }
                if age < self.max_stale {
                    if !entry.refreshing {
                        entry.refreshing = true;
                        let cache = self.clone();
                        let key = key.to_string();
                        tokio::spawn(async move {
                            let result = compute().await;
                            cache.store(&key, result);
                        });
                    }
                    return Ok(entry.value.clone());
                }
            }
        }
        let value = compute().await?;
        self.store::<E>(key, Ok(value.clone()));
        Ok(value)
    }

    fn store<E: std::fmt::Display>(&self, key: &str, result: Result<Value, E>) {
        let mut entries = self.entries.lock().unwrap();
        match result {
            Ok(value) => {
                entries.insert(
                    key.to_string(),
                    Entry {
                        value,
                        computed: Instant::now(),
                        refreshing: false,
                    },
                );
            }
            Err(e) => {
                tracing::warn!(key, "background refresh failed: {e}");
                // Keep serving the stale value; the next request tries again.
                if let Some(entry) = entries.get_mut(key) {
                    entry.refreshing = false;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use serde_json::json;

    use super::*;

    type Computation = std::pin::Pin<Box<dyn Future<Output = Result<Value, String>> + Send>>;

    /// A computation that returns how many times it has run.
    fn counter() -> (Arc<AtomicU64>, impl Fn() -> Computation + Clone) {
        let calls = Arc::new(AtomicU64::new(0));
        let c = calls.clone();
        let compute = move || {
            let c = c.clone();
            Box::pin(async move { Ok(json!(c.fetch_add(1, Ordering::SeqCst) + 1)) }) as Computation
        };
        (calls, compute)
    }

    #[tokio::test]
    async fn fresh_values_are_reused_and_stale_ones_refresh_in_the_background() {
        let cache = ResponseCache::new(Duration::from_millis(40), Duration::from_secs(60));
        let (calls, compute) = counter();

        assert_eq!(cache.get("k", compute.clone()).await.unwrap(), json!(1));
        assert_eq!(cache.get("k", compute.clone()).await.unwrap(), json!(1));
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        tokio::time::sleep(Duration::from_millis(60)).await;
        // Stale: served at once, recomputed behind the caller.
        assert_eq!(cache.get("k", compute.clone()).await.unwrap(), json!(1));
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert_eq!(cache.get("k", compute.clone()).await.unwrap(), json!(2));
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn too_old_values_and_a_zero_ttl_compute_inline() {
        let cache = ResponseCache::new(Duration::from_millis(10), Duration::from_millis(20));
        let (_, compute) = counter();
        assert_eq!(cache.get("k", compute.clone()).await.unwrap(), json!(1));
        tokio::time::sleep(Duration::from_millis(30)).await;
        assert_eq!(cache.get("k", compute.clone()).await.unwrap(), json!(2));

        let off = ResponseCache::new(Duration::ZERO, Duration::ZERO);
        let (calls, compute) = counter();
        off.get("k", compute.clone()).await.unwrap();
        off.get("k", compute.clone()).await.unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }
}
