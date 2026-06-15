use crate::types::{DepthUpdate, OrderbookSnapshot};
use anyhow::Result;
use std::collections::BTreeMap;
use tracing::{info, warn};

pub struct Orderbook {
    symbol: String,
    bids: BTreeMap<u64, f64>,
    asks: BTreeMap<u64, f64>,
    last_update_id: u64,
    depth_levels: usize,
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
            last_update_id: 0,
            depth_levels,
        }
    }

    pub fn reset(&mut self) {
        self.bids.clear();
        self.asks.clear();
        self.last_update_id = 0;
        info!(symbol = %self.symbol, "Orderbook reset");
    }

    pub fn apply_snapshot(&mut self, snapshot_last_id: u64, bids: &[[String; 2]], asks: &[[String; 2]]) {
        self.reset();
        self.last_update_id = snapshot_last_id;
        for level in bids {
            let price: f64 = level[0].parse().unwrap_or(0.0);
            let qty: f64 = level[1].parse().unwrap_or(0.0);
            if qty > 0.0 {
                self.bids.insert(price_to_key(price), qty);
            }
        }
        for level in asks {
            let price: f64 = level[0].parse().unwrap_or(0.0);
            let qty: f64 = level[1].parse().unwrap_or(0.0);
            if qty > 0.0 {
                self.asks.insert(price_to_key(price), qty);
            }
        }
        info!(symbol = %self.symbol, last_update_id = snapshot_last_id, "Snapshot applied");
    }

    pub fn apply_update(&mut self, update: &DepthUpdate) -> Result<bool> {
        if self.last_update_id == 0 {
            return Ok(false);
        }

        if update.prev_last_update_id != self.last_update_id {
            warn!(
                expected = self.last_update_id,
                got = update.prev_last_update_id,
                "Sequence gap detected, reset required"
            );
            return Ok(false);
        }

        for level in &update.bids {
            let price: f64 = level[0].parse().unwrap_or(0.0);
            let qty: f64 = level[1].parse().unwrap_or(0.0);
            let key = price_to_key(price);
            if qty == 0.0 {
                self.bids.remove(&key);
            } else {
                self.bids.insert(key, qty);
            }
        }
        for level in &update.asks {
            let price: f64 = level[0].parse().unwrap_or(0.0);
            let qty: f64 = level[1].parse().unwrap_or(0.0);
            let key = price_to_key(price);
            if qty == 0.0 {
                self.asks.remove(&key);
            } else {
                self.asks.insert(key, qty);
            }
        }
        self.last_update_id = update.last_update_id;
        Ok(true)
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
}
