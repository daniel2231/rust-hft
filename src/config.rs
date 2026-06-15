use anyhow::Result;
use serde::Deserialize;
use std::fs;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub mode: String,
    pub symbol: String,
    pub risk: RiskConfig,
    pub orderbook: OrderbookConfig,
    pub exchange: ExchangeConfig,
    pub watchdog: WatchdogConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RiskConfig {
    pub max_order_qty: f64,
    pub min_free_balance_usdt: f64,
    pub price_band_pct: f64,
    pub max_orders_per_sec: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct OrderbookConfig {
    pub depth_levels: usize,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ExchangeConfig {
    pub ws_url: String,
    pub rest_url: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct WatchdogConfig {
    pub timeout_secs: u64,
}

pub fn load_config(path: &str) -> Result<Config> {
    let content = fs::read_to_string(path)?;
    let config: Config = toml::from_str(&content)?;
    Ok(config)
}
