use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct DepthUpdate {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "E")]
    pub event_time: u64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "U")]
    pub first_update_id: u64,
    #[serde(rename = "u")]
    pub last_update_id: u64,
    #[serde(rename = "pu")]
    pub prev_last_update_id: u64,
    #[serde(rename = "b")]
    pub bids: Vec<[String; 2]>,
    #[serde(rename = "a")]
    pub asks: Vec<[String; 2]>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Trade {
    #[serde(rename = "e")]
    pub event_type: String,
    #[serde(rename = "E")]
    pub event_time: u64,
    #[serde(rename = "s")]
    pub symbol: String,
    #[serde(rename = "p")]
    pub price: String,
    #[serde(rename = "q")]
    pub qty: String,
    #[serde(rename = "m")]
    pub is_buyer_maker: bool,
}

#[derive(Debug, Clone)]
pub struct DepthSnapshot {
    pub last_update_id: u64,
    pub bids: Vec<[String; 2]>,
    pub asks: Vec<[String; 2]>,
}

#[derive(Debug, Clone)]
pub enum MarketEvent {
    DepthUpdate(DepthUpdate),
    DepthSnapshot(DepthSnapshot),
    Trade(Trade),
    Reconnect,
}

#[derive(Debug, Clone)]
pub struct OrderbookSnapshot {
    pub symbol: String,
    pub bids: Vec<(f64, f64)>,
    pub asks: Vec<(f64, f64)>,
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone)]
pub enum Signal {
    Buy { price: f64, qty: f64 },
    Sell { price: f64, qty: f64 },
    Hold,
}

#[derive(Debug, Clone)]
pub struct ValidatedOrder {
    pub signal: Signal,
    pub client_order_id: String,
}
