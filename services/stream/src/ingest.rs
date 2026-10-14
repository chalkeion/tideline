//! Chain ingestor: follows the Arc head, turning each block range into stored flows.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use sqlx::PgPool;
use tokio::sync::broadcast;

use crate::config::Config;
use crate::db;
use crate::decode::{decode, watched_topics, Decoded};
use crate::enrich::build_flows;
use crate::rates::LiveRates;
use crate::rpc::{RawLog, RpcClient};
use crate::schema::IngestStatus;

/// Shared, lock-free view of ingest progress for the API.
#[derive(Default)]
pub struct Progress {
    running: AtomicBool,
    /// Last fully ingested block; -1 until the first range lands.
    last_block: AtomicI64,
    chain_head: AtomicI64,
    last_error: Mutex<Option<String>>,
}

impl Progress {
    pub fn new() -> Self {
        Self {
            last_block: AtomicI64::new(-1),
            chain_head: AtomicI64::new(-1),
            ..Default::default()
        }
    }

    pub fn snapshot(&self) -> IngestStatus {
        let known = |v: i64| (v >= 0).then_some(v);
        let last_block = known(self.last_block.load(Ordering::Relaxed));
        let chain_head = known(self.chain_head.load(Ordering::Relaxed));
        IngestStatus {
            running: self.running.load(Ordering::Relaxed),
            last_block,
            chain_head,
            lag_blocks: last_block.zip(chain_head).map(|(l, h)| (h - l).max(0)),
            last_error: self.last_error.lock().unwrap().clone(),
        }
    }

    fn set_error(&self, error: Option<String>) {
        *self.last_error.lock().unwrap() = error;
    }
}

pub struct Ingestor {
    pub cfg: Arc<Config>,
    pub pool: PgPool,
    pub rpc: RpcClient,
    pub rates: Arc<LiveRates>,
    pub progress: Arc<Progress>,
    /// Ids of newly stored flows, for live subscribers.
    pub new_flows: broadcast::Sender<Vec<i64>>,
}

impl Ingestor {
    /// Follow the chain until the task is dropped. Errors are logged and retried.
    pub async fn run(self) {
        self.progress.running.store(true, Ordering::Relaxed);
        let mut chunk = self.cfg.max_chunk_blocks.min(20);
        // Set once backfill first reaches the head. Blocks above it were produced while
        // tip-following, and all of them count toward the latency claim from then on,
        // including any the ingestor is slow to pick up.
        let mut live_after: Option<u64> = None;
        let mut at_tip = false;
        let mut backoff = Duration::from_secs(1);

        loop {
            match self.step(&mut chunk, live_after, at_tip).await {
                Ok(head_reached) => {
                    self.progress.set_error(None);
                    backoff = Duration::from_secs(1);
                    at_tip = head_reached.is_some();
                    if let Some(head) = head_reached {
                        live_after.get_or_insert(head);
                        tokio::time::sleep(Duration::from_millis(self.cfg.poll_interval_ms)).await;
                    }
                }
                Err(e) => {
                    tracing::warn!("ingest step failed: {e:#}");
                    self.progress.set_error(Some(format!("{e:#}")));
                    // A range that is too heavy for the node is the usual cause: shrink it.
                    chunk = (chunk / 2).max(1);
                    at_tip = false;
                    tokio::time::sleep(backoff).await;
                    backoff = (backoff * 2).min(Duration::from_secs(30));
                }
            }
        }
    }

    /// Ingest one block range. Returns the head when the range reached it.
    async fn step(
        &self,
        chunk: &mut u64,
        live_after: Option<u64>,
        at_tip: bool,
    ) -> anyhow::Result<Option<u64>> {
        let cfg = &self.cfg;
        let addresses = cfg.contracts.watched();
        let topics = watched_topics(cfg.confidential_transfer_topic);
        let cursor = db::load_cursor(&self.pool).await?.map(|next| next as u64);

        // At the tip with no confirmation depth, head and logs come back together.
        let (safe_head, from, to, logs) = match cursor {
            Some(from) if at_tip && cfg.confirmations == 0 => {
                let (head, logs) = self.rpc.head_and_logs(from, &addresses, &topics).await?;
                self.progress
                    .chain_head
                    .store(head as i64, Ordering::Relaxed);
                if from > head {
                    return Ok(Some(head));
                }
                (head, from, head, logs)
            }
            _ => {
                let head = self.rpc.block_number().await?;
                self.progress
                    .chain_head
                    .store(head as i64, Ordering::Relaxed);
                let safe_head = head.saturating_sub(cfg.confirmations);
                let from = cursor.unwrap_or(safe_head.saturating_sub(cfg.backfill_blocks));
                if from > safe_head {
                    return Ok(Some(safe_head));
                }
                let to = (from + *chunk - 1).min(safe_head);
                let logs = self.rpc.get_logs(from, to, &addresses, &topics).await?;
                (safe_head, from, to, logs)
            }
        };

        // Until backfill has reached the head once, nothing is live.
        let stored = self
            .store_range(&logs, to, live_after.unwrap_or(u64::MAX))
            .await?;
        self.progress.last_block.store(to as i64, Ordering::Relaxed);
        tracing::info!(
            from,
            to,
            logs = logs.len(),
            flows = stored,
            "ingested range"
        );

        // Grow the range while the node keeps up, so backfill converges on the head.
        if to < safe_head {
            *chunk = (*chunk * 2).min(cfg.max_chunk_blocks);
        }
        Ok((to >= safe_head).then_some(safe_head))
    }

    async fn store_range(
        &self,
        logs: &[RawLog],
        to: u64,
        live_after: u64,
    ) -> anyhow::Result<usize> {
        let cfg = &self.cfg;
        let events: Vec<Decoded> = logs
            .iter()
            .filter(|l| !l.removed)
            .filter_map(|l| decode(l, &cfg.contracts, cfg.confidential_transfer_topic))
            .collect();
        let (flows, links) = build_flows(&events, &cfg.contracts, &self.rates.current());

        let timestamps = self.block_timestamps(logs, &flows).await?;
        let rows: Vec<_> = flows
            .into_iter()
            .map(|f| {
                let t = timestamps[&(f.block_number as u64)] as i64;
                (f, t)
            })
            .collect();

        // Flows and cursor commit together: a crash can never skip or half-apply a range.
        let mut tx = self.pool.begin().await?;
        let live_after = i64::try_from(live_after).unwrap_or(i64::MAX);
        let ids = db::insert_flows(&mut tx, &rows, live_after).await?;
        db::insert_links(&mut tx, &links).await?;
        db::save_cursor(&mut tx, to as i64 + 1).await?;
        tx.commit().await?;

        if !links.is_empty() {
            db::propagate_links(&self.pool).await?;
        }
        let stored = ids.len();
        if !ids.is_empty() {
            // No subscribers is fine.
            let _ = self.new_flows.send(ids);
        }
        Ok(stored)
    }

    /// Timestamp for every block that produced a flow, from the logs when the node
    /// supplies it and from the block header otherwise.
    async fn block_timestamps(
        &self,
        logs: &[RawLog],
        flows: &[crate::enrich::NewFlow],
    ) -> anyhow::Result<HashMap<u64, u64>> {
        let mut out: HashMap<u64, u64> = logs
            .iter()
            .filter_map(|l| Some((l.block_number.to::<u64>(), l.block_timestamp?.to::<u64>())))
            .collect();
        for f in flows {
            let block = f.block_number as u64;
            if let std::collections::hash_map::Entry::Vacant(slot) = out.entry(block) {
                slot.insert(self.rpc.block_timestamp(block).await?);
            }
        }
        Ok(out)
    }
}
