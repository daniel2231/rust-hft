use crate::types::{Signal, ValidatedOrder};
use std::collections::HashSet;
use std::time::{Duration, Instant};
use tracing::{error, warn};
use uuid::Uuid;

pub struct RiskChecker {
    pub halted: bool,
    pub max_order_qty: f64,
    pub min_free_balance_usdt: f64,
    pub price_band_pct: f64,
    pub max_orders_per_sec: u32,
    seen_order_ids: HashSet<String>,
    order_timestamps: Vec<Instant>,
    pub free_balance_usdt: f64,
}

impl RiskChecker {
    pub fn new(
        max_order_qty: f64,
        min_free_balance_usdt: f64,
        price_band_pct: f64,
        max_orders_per_sec: u32,
    ) -> Self {
        Self {
            halted: false,
            max_order_qty,
            min_free_balance_usdt,
            price_band_pct,
            max_orders_per_sec,
            seen_order_ids: HashSet::new(),
            order_timestamps: Vec::new(),
            free_balance_usdt: 10_000.0,
        }
    }

    pub fn check(&mut self, signal: &Signal, mid_price: f64) -> Option<ValidatedOrder> {
        if self.halted {
            error!("Kill switch active — order blocked");
            return None;
        }

        let (price, qty) = match signal {
            Signal::Buy { price, qty } => (*price, *qty),
            Signal::Sell { price, qty } => (*price, *qty),
            Signal::Hold => return None,
        };

        if qty > self.max_order_qty {
            warn!(qty, max = self.max_order_qty, "Order qty exceeds max — blocked");
            return None;
        }

        if self.free_balance_usdt < self.min_free_balance_usdt {
            warn!(balance = self.free_balance_usdt, "Insufficient balance — blocked");
            return None;
        }

        if mid_price > 0.0 {
            let pct_diff = ((price - mid_price) / mid_price).abs() * 100.0;
            if pct_diff > self.price_band_pct {
                warn!(price, mid_price, pct_diff, "Fat-finger price check failed — blocked");
                return None;
            }
        }

        let now = Instant::now();
        self.order_timestamps.retain(|t| now.duration_since(*t) < Duration::from_secs(1));
        if self.order_timestamps.len() as u32 >= self.max_orders_per_sec {
            warn!("Rate limit exceeded — blocked");
            return None;
        }

        let client_order_id = Uuid::new_v4().to_string();
        if self.seen_order_ids.contains(&client_order_id) {
            error!(id = %client_order_id, "Duplicate order ID — blocked");
            return None;
        }

        self.seen_order_ids.insert(client_order_id.clone());
        self.order_timestamps.push(now);

        Some(ValidatedOrder {
            signal: signal.clone(),
            client_order_id,
        })
    }
}
