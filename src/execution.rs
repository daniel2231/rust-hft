use crate::types::{Signal, ValidatedOrder};
use chrono::Utc;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tracing::{error, info};

pub struct PaperExecutor {
    symbol: String,
    /// Opened once at startup and held for the process lifetime, so each
    /// order costs one write syscall instead of open+write+close.
    log_file: Mutex<Option<File>>,
    buy_count: AtomicU64,
    sell_count: AtomicU64,
    buy_volume: Mutex<f64>,
    sell_volume: Mutex<f64>,
}

impl PaperExecutor {
    pub fn new(symbol: String) -> Self {
        std::fs::create_dir_all("logs").ok();
        let log_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open("logs/orders_paper.log")
            .map_err(|e| error!("Failed to open paper log: {}", e))
            .ok();
        Self {
            symbol,
            log_file: Mutex::new(log_file),
            buy_count: AtomicU64::new(0),
            sell_count: AtomicU64::new(0),
            buy_volume: Mutex::new(0.0),
            sell_volume: Mutex::new(0.0),
        }
    }

    pub fn execute(&self, order: &ValidatedOrder) {
        let (side, price, qty) = match &order.signal {
            Signal::Buy { price, qty } => ("BUY", price, qty),
            Signal::Sell { price, qty } => ("SELL", price, qty),
            Signal::Hold => return,
        };

        let signal_str = match &order.signal {
            Signal::Buy { .. } => "Buy",
            Signal::Sell { .. } => "Sell",
            Signal::Hold => return,
        };

        // Update stats
        match &order.signal {
            Signal::Buy { qty, .. } => {
                self.buy_count.fetch_add(1, Ordering::Relaxed);
                if let Ok(mut vol) = self.buy_volume.lock() {
                    *vol += qty;
                }
            }
            Signal::Sell { qty, .. } => {
                self.sell_count.fetch_add(1, Ordering::Relaxed);
                if let Ok(mut vol) = self.sell_volume.lock() {
                    *vol += qty;
                }
            }
            Signal::Hold => {}
        }

        let line = format!(
            "[PAPER] {} | {} | {} | qty={:.4} | price={:.2} | signal={} | client_id={}\n",
            Utc::now().format("%Y-%m-%dT%H:%M:%SZ"),
            side,
            self.symbol,
            qty,
            price,
            signal_str,
            order.client_order_id,
        );

        info!("{}", line.trim());

        match self.log_file.lock() {
            Ok(mut guard) => match guard.as_mut() {
                Some(file) => {
                    if let Err(e) = file.write_all(line.as_bytes()) {
                        error!("Failed to write to paper log: {}", e);
                    }
                }
                None => error!("Paper log unavailable — order not persisted"),
            },
            Err(_) => error!("Paper log mutex poisoned — order not persisted"),
        }
    }

    pub fn execute_with_state(&self, order: &ValidatedOrder, state: &Arc<crate::dashboard::SharedState>) {
        self.execute(order);

        let (side, price, qty) = match &order.signal {
            Signal::Buy { price, qty } => ("BUY", *price, *qty),
            Signal::Sell { price, qty } => ("SELL", *price, *qty),
            Signal::Hold => return,
        };

        let trade = crate::dashboard::PaperTrade {
            timestamp: Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
            side: side.to_string(),
            price,
            qty,
            client_id: order.client_order_id.clone(),
        };

        match &order.signal {
            Signal::Buy { .. } => {
                state.buy_count.fetch_add(1, Ordering::Relaxed);
            }
            Signal::Sell { .. } => {
                state.sell_count.fetch_add(1, Ordering::Relaxed);
            }
            Signal::Hold => {}
        }

        state.add_trade(trade);
    }

    pub fn log_stats(&self) {
        let buy_count = self.buy_count.load(Ordering::Relaxed);
        let sell_count = self.sell_count.load(Ordering::Relaxed);
        let buy_volume = self.buy_volume.lock().map(|v| *v).unwrap_or(0.0);
        let sell_volume = self.sell_volume.lock().map(|v| *v).unwrap_or(0.0);

        info!(
            buy_count,
            sell_count,
            buy_volume,
            sell_volume,
            "Paper trading stats: buys={} sells={} buy_vol={:.4} sell_vol={:.4}",
            buy_count,
            sell_count,
            buy_volume,
            sell_volume,
        );
    }
}
