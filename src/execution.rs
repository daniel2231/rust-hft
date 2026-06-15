use crate::types::{Signal, ValidatedOrder};
use chrono::Utc;
use std::fs::OpenOptions;
use std::io::Write;
use tracing::{error, info};

pub struct PaperExecutor {
    symbol: String,
    log_path: String,
}

impl PaperExecutor {
    pub fn new(symbol: String) -> Self {
        std::fs::create_dir_all("logs").ok();
        Self {
            symbol,
            log_path: "logs/orders_paper.log".to_string(),
        }
    }

    pub fn execute(&self, order: &ValidatedOrder) {
        let (side, price, qty) = match &order.signal {
            Signal::Buy { price, qty } => ("BUY", price, qty),
            Signal::Sell { price, qty } => ("SELL", price, qty),
            Signal::Hold => return,
        };

        let line = format!(
            "[PAPER] {} | {} | {} | qty={:.4} | price={:.2} | client_id={}\n",
            Utc::now().to_rfc3339(),
            side,
            self.symbol,
            qty,
            price,
            order.client_order_id,
        );

        info!("{}", line.trim());

        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&self.log_path) {
            let _ = file.write_all(line.as_bytes());
        } else {
            error!("Failed to write to paper log");
        }
    }
}
