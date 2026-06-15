use crate::types::{DepthSnapshot, DepthUpdate, OrderbookSnapshot};
use anyhow::Result;
use std::collections::BTreeMap;
use tracing::{error, info, warn};

enum SyncState {
    Buffering(Vec<DepthUpdate>),
    Live { last_update_id: u64 },
}

pub struct Orderbook {
    symbol: String,
    bids: BTreeMap<u64, f64>,
    asks: BTreeMap<u64, f64>,
    depth_levels: usize,
    state: SyncState,
}

fn price_to_key(price: f64) -> u64 {
    (price * 1e8) as u64
}

impl Orderbook {
    pub fn new(symbol: String, depth_levels: usize) -> Self {
        Self {
            symbol,
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            depth_levels,
            state: SyncState::Buffering(Vec::new()),
        }
    }

    pub fn reset(&mut self) {
        self.bids.clear();
        self.asks.clear();
        self.state = SyncState::Buffering(Vec::new());
        info!(symbol = %self.symbol, "Orderbook reset to Buffering");
    }

    fn apply_levels(bids: &mut BTreeMap<u64, f64>, asks: &mut BTreeMap<u64, f64>, update: &DepthUpdate) {
        for level in &update.bids {
            let price: f64 = level[0].parse().unwrap_or(0.0);
            let qty: f64 = level[1].parse().unwrap_or(0.0);
            let key = price_to_key(price);
            if qty == 0.0 {
                bids.remove(&key);
            } else {
                bids.insert(key, qty);
            }
        }
        for level in &update.asks {
            let price: f64 = level[0].parse().unwrap_or(0.0);
            let qty: f64 = level[1].parse().unwrap_or(0.0);
            let key = price_to_key(price);
            if qty == 0.0 {
                asks.remove(&key);
            } else {
                asks.insert(key, qty);
            }
        }
    }

    /// Returns true if the orderbook is live (synced), false if still buffering.
    pub fn handle_snapshot(&mut self, snapshot: &DepthSnapshot) -> bool {
        let s = snapshot.last_update_id;

        let buffered = match &self.state {
            SyncState::Buffering(buf) => buf.clone(),
            SyncState::Live { .. } => {
                // Re-sync: treat as fresh buffering with empty buffer
                Vec::new()
            }
        };

        // Find first event where event.U <= S+1 AND event.u >= S+1
        let first_valid = buffered.iter().position(|e| {
            e.first_update_id <= s + 1 && e.last_update_id >= s + 1
        });

        // Apply snapshot to BTreeMap
        self.bids.clear();
        self.asks.clear();
        for level in &snapshot.bids {
            let price: f64 = level[0].parse().unwrap_or(0.0);
            let qty: f64 = level[1].parse().unwrap_or(0.0);
            if qty > 0.0 {
                self.bids.insert(price_to_key(price), qty);
            }
        }
        for level in &snapshot.asks {
            let price: f64 = level[0].parse().unwrap_or(0.0);
            let qty: f64 = level[1].parse().unwrap_or(0.0);
            if qty > 0.0 {
                self.asks.insert(price_to_key(price), qty);
            }
        }

        info!(symbol = %self.symbol, last_update_id = s, "Snapshot applied");

        let valid_events: Vec<DepthUpdate> = match first_valid {
            Some(idx) => buffered[idx..].to_vec(),
            None => {
                self.state = SyncState::Live { last_update_id: s };
                info!(symbol = %self.symbol, "Live (no buffered events to apply)");
                return true;
            }
        };

        // Apply buffered events starting from the first valid one.
        // pu continuity is checked only between consecutive events, NOT against the snapshot ID.
        let mut last_id = s;
        let mut first = true;
        for event in &valid_events {
            if first {
                // First event: condition U<=S+1 AND u>=S+1 already verified above.
                // pu may not equal S — that's OK per Binance spec.
                first = false;
            } else if event.prev_last_update_id != last_id {
                warn!(
                    expected = last_id,
                    got = event.prev_last_update_id,
                    "Sequence gap in buffered events during snapshot apply — stopping"
                );
                break;
            }
            Self::apply_levels(&mut self.bids, &mut self.asks, event);
            last_id = event.last_update_id;
        }

        self.state = SyncState::Live { last_update_id: last_id };
        info!(symbol = %self.symbol, last_update_id = last_id, "Orderbook Live");
        true
    }

    /// Returns true if update was applied, false if buffering or sequence gap.
    /// Sets resync_needed=true on sequence gap so ingestion re-fetches snapshot.
    pub fn handle_update(&mut self, update: &DepthUpdate, resync_needed: &std::sync::atomic::AtomicBool) -> Result<bool> {
        match &self.state {
            SyncState::Buffering(buf) => {
                let mut buf = buf.clone();
                buf.push(update.clone());
                self.state = SyncState::Buffering(buf);
                Ok(false)
            }
            SyncState::Live { last_update_id } => {
                let expected = *last_update_id;
                if update.prev_last_update_id != expected {
                    if update.last_update_id <= expected {
                        // Fully stale: entire event range is before our state — skip
                        tracing::debug!(
                            expected,
                            event_u = update.last_update_id,
                            "Skipping stale event"
                        );
                        return Ok(false);
                    }
                    if update.prev_last_update_id < expected && update.last_update_id > expected {
                        // Event range COVERS our expected ID — apply and advance
                        tracing::debug!(
                            expected,
                            event_pu = update.prev_last_update_id,
                            event_u = update.last_update_id,
                            "Applying spanning event"
                        );
                    } else {
                        // True gap: events were missed
                        error!(
                            expected,
                            got = update.prev_last_update_id,
                            "Sequence gap detected — resetting to Buffering"
                        );
                        self.reset();
                        resync_needed.store(true, std::sync::atomic::Ordering::Relaxed);
                        return Ok(false);
                    }
                }
                Self::apply_levels(&mut self.bids, &mut self.asks, update);
                self.state = SyncState::Live { last_update_id: update.last_update_id };
                Ok(true)
            }
        }
    }

    /// Legacy method kept for compatibility — delegates to handle_update.
    pub fn apply_update(&mut self, update: &DepthUpdate) -> Result<bool> {
        self.handle_update(update, &std::sync::atomic::AtomicBool::new(false))
    }

    pub fn snapshot(&self) -> OrderbookSnapshot {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let bids: Vec<(f64, f64)> = self.bids.iter().rev()
            .take(self.depth_levels)
            .map(|(&k, &qty)| (k as f64 / 1e8, qty))
            .collect();

        let asks: Vec<(f64, f64)> = self.asks.iter()
            .take(self.depth_levels)
            .map(|(&k, &qty)| (k as f64 / 1e8, qty))
            .collect();

        OrderbookSnapshot {
            symbol: self.symbol.clone(),
            bids,
            asks,
            timestamp_ms: now,
        }
    }

    pub fn is_live(&self) -> bool {
        matches!(self.state, SyncState::Live { .. })
    }
}
