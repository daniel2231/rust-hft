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
    /// Tunable parameters for the momentum/pingpong strategies.
    #[serde(default)]
    pub momentum: MomentumConfig,
    #[serde(default)]
    pub pingpong: PingPongConfig,
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
pub struct MomentumConfig {
    /// Order quantity per entry (base asset units), before capital sizing.
    pub qty: f64,
    /// Number of ~100ms orderbook ticks to look back for the momentum check.
    pub lookback_ticks: usize,
    /// Minimum mid-price rise over the lookback window to trigger entry (%).
    pub entry_mom_pct: f64,
    /// Minimum fraction (0.0-1.0) of top-5-level volume on the bid side to
    /// confirm entry.
    pub imbalance_min: f64,
    /// Take-profit distance from entry price (%).
    pub tp_pct: f64,
    /// Stop-loss distance from entry price (%).
    pub stop_pct: f64,
    /// Force-close a position after this many ticks even without a
    /// take-profit or stop-loss hit.
    pub max_hold_ticks: u32,
    /// Ticks to wait after closing a position before allowing re-entry.
    pub cooldown_ticks: u32,
}

impl Default for MomentumConfig {
    fn default() -> Self {
        Self {
            qty: 0.01,
            lookback_ticks: 30,
            entry_mom_pct: 0.05,
            imbalance_min: 0.60,
            // Take-profit must clear the round-trip cost (taker fee 0.05% ×2
            // = 0.10% plus spread) or every "win" is a net loss.
            tp_pct: 0.20,
            stop_pct: 0.10,
            max_hold_ticks: 300,
            cooldown_ticks: 30,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct PingPongConfig {
    pub qty: f64,
    pub min_edge_pct: f64,
    pub stop_pct: f64,
}

impl Default for PingPongConfig {
    fn default() -> Self {
        Self {
            qty: 0.01,
            min_edge_pct: 0.02,
            stop_pct: 0.5,
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
