use anyhow::Result;
use serde::Deserialize;
use std::fs;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub mode: String,
    pub symbol: String,
    /// Strategy to run in crypto-trader ("noop" | "pingpong").
    #[serde(default = "default_strategy")]
    pub strategy: String,
    /// Paper-trading virtual account settings.
    #[serde(default)]
    pub paper: PaperConfig,
    pub risk: RiskConfig,
    pub orderbook: OrderbookConfig,
    pub exchange: ExchangeConfig,
    pub watchdog: WatchdogConfig,
    pub dashboard: DashboardConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PaperConfig {
    /// Virtual starting capital for the paper account (USDT).
    pub initial_capital_usdt: f64,
    /// Simulated fee per fill, percent of notional (Binance futures maker
    /// fee is 0.02, taker 0.05).
    pub fee_pct: f64,
}

impl Default for PaperConfig {
    fn default() -> Self {
        Self {
            initial_capital_usdt: 10_000.0,
            fee_pct: 0.02,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct DashboardConfig {
    pub port: u16,
    pub enabled: bool,
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

fn default_strategy() -> String {
    "noop".to_string()
}

pub fn load_config(path: &str) -> Result<Config> {
    let content = fs::read_to_string(path)?;
    let config: Config = toml::from_str(&content)?;
    Ok(config)
}
