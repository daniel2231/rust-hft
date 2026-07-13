use crate::types::{Signal, ValidatedOrder};
use std::collections::{HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use tracing::{error, warn};
use uuid::Uuid;

/// Cap on the duplicate-order-ID window so memory stays bounded over long runs.
const MAX_SEEN_ORDER_IDS: usize = 10_000;

#[derive(Debug, Error)]
pub enum RiskError {
    #[error("Kill switch active")]
    KillSwitch,
    #[error("Order qty {qty} exceeds max {max}")]
    MaxQtyExceeded { qty: f64, max: f64 },
    #[error("Insufficient balance: {balance:.2} USDT (min {min:.2})")]
    InsufficientBalance { balance: f64, min: f64 },
    #[error("Insufficient funds: order notional {notional:.2} USDT exceeds balance {balance:.2}")]
    InsufficientFunds { notional: f64, balance: f64 },
    #[error("Fat-finger: price {price:.2} deviates {pct:.2}% from mid {mid:.2} (max {max_pct:.1}%)")]
    FatFinger { price: f64, mid: f64, pct: f64, max_pct: f64 },
    #[error("Rate limit: {current}/{max} orders/sec")]
    RateLimit { current: usize, max: u32 },
    #[error("Duplicate order ID: {id}")]
    DuplicateOrderId { id: String },
}

pub struct RiskChecker {
    pub halted: bool,
    /// Kill-switch flag maintained by a background HALT-file poller.
    /// Read atomically per check so the hot path never touches the filesystem.
    halt_flag: Option<Arc<AtomicBool>>,
    /// Live cash balance (f64 bits) mirrored from the paper account after
    /// each fill. When set, it replaces the static `free_balance_usdt`.
    balance_source: Option<Arc<std::sync::atomic::AtomicU64>>,
    pub max_order_qty: f64,
    pub min_free_balance_usdt: f64,
    pub price_band_pct: f64,
    pub max_orders_per_sec: u32,
    seen_order_ids: HashSet<String>,
    seen_order_queue: VecDeque<String>,
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
            halt_flag: None,
            balance_source: None,
            max_order_qty,
            min_free_balance_usdt,
            price_band_pct,
            max_orders_per_sec,
            seen_order_ids: HashSet::new(),
            seen_order_queue: VecDeque::new(),
            order_timestamps: Vec::new(),
            free_balance_usdt: 10_000.0,
        }
    }

    /// Wire up the shared kill-switch flag (see the HALT poller in main).
    pub fn with_halt_flag(mut self, flag: Arc<AtomicBool>) -> Self {
        self.halt_flag = Some(flag);
        self
    }

    /// Wire up the live paper-account balance (f64 bits). Balance checks
    /// then track the simulated account instead of a static number.
    pub fn with_balance_source(mut self, source: Arc<std::sync::atomic::AtomicU64>) -> Self {
        self.balance_source = Some(source);
        self
    }

    pub fn check(&mut self, signal: &Signal, mid_price: f64) -> Result<ValidatedOrder, RiskError> {
        let file_halt = self
            .halt_flag
            .as_ref()
            .is_some_and(|f| f.load(Ordering::Relaxed));

        if self.halted || file_halt {
            error!("Kill switch active — order blocked");
            return Err(RiskError::KillSwitch);
        }

        let (price, qty, is_buy) = match signal {
            Signal::Buy { price, qty } => (*price, *qty, true),
            Signal::Sell { price, qty } => (*price, *qty, false),
            Signal::Hold => unreachable!("Hold signals should not be passed to check()"),
        };

        if qty > self.max_order_qty {
            warn!(qty, max = self.max_order_qty, "Order qty exceeds max — blocked");
            return Err(RiskError::MaxQtyExceeded { qty, max: self.max_order_qty });
        }

        // Balance checks apply to buys only — sells release cash, and
        // blocking them would strand an open position when the balance is
        // already below the floor.
        if is_buy {
            let balance = self
                .balance_source
                .as_ref()
                .map(|s| f64::from_bits(s.load(Ordering::Relaxed)))
                .unwrap_or(self.free_balance_usdt);

            if balance < self.min_free_balance_usdt {
                warn!(balance, "Insufficient balance — blocked");
                return Err(RiskError::InsufficientBalance {
                    balance,
                    min: self.min_free_balance_usdt,
                });
            }

            // 0.1% headroom so the simulated fill fee can't push cash negative.
            let notional = price * qty * 1.001;
            if notional > balance {
                warn!(notional, balance, "Order notional exceeds balance — blocked");
                return Err(RiskError::InsufficientFunds { notional, balance });
            }
        }

        if mid_price > 0.0 {
            let pct_diff = ((price - mid_price) / mid_price).abs() * 100.0;
            if pct_diff > self.price_band_pct {
                warn!(price, mid_price, pct_diff, "Fat-finger price check failed — blocked");
                return Err(RiskError::FatFinger {
                    price,
                    mid: mid_price,
                    pct: pct_diff,
                    max_pct: self.price_band_pct,
                });
            }
        }

        let now = Instant::now();
        self.order_timestamps.retain(|t| now.duration_since(*t) < Duration::from_secs(1));
        let current_rate = self.order_timestamps.len();

        // Warn when >= 80% of rate limit
        let warn_threshold = (self.max_orders_per_sec as f64 * 0.8).floor() as usize;
        if current_rate >= warn_threshold {
            warn!(
                current = current_rate,
                max = self.max_orders_per_sec,
                "Order rate near limit: {}/{} orders/sec",
                current_rate,
                self.max_orders_per_sec
            );
        }

        if current_rate as u32 >= self.max_orders_per_sec {
            warn!("Rate limit exceeded — blocked");
            return Err(RiskError::RateLimit {
                current: current_rate,
                max: self.max_orders_per_sec,
            });
        }

        let client_order_id = Uuid::new_v4().to_string();
        if self.seen_order_ids.contains(&client_order_id) {
            error!(id = %client_order_id, "Duplicate order ID — blocked");
            return Err(RiskError::DuplicateOrderId { id: client_order_id });
        }

        if self.seen_order_queue.len() >= MAX_SEEN_ORDER_IDS {
            if let Some(oldest) = self.seen_order_queue.pop_front() {
                self.seen_order_ids.remove(&oldest);
            }
        }
        self.seen_order_ids.insert(client_order_id.clone());
        self.seen_order_queue.push_back(client_order_id.clone());
        self.order_timestamps.push(now);

        Ok(ValidatedOrder {
            signal: signal.clone(),
            client_order_id,
        })
    }
}
