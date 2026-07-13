pub mod server;

use crate::pnl::FifoPnl;
use crate::types::OrderbookSnapshot;
use parking_lot::RwLock;
use serde::Serialize;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

#[derive(Clone, Serialize)]
pub struct PaperTrade {
    pub timestamp: String,
    pub side: String,
    pub price: f64,
    pub qty: f64,
    pub client_id: String,
}

#[derive(Clone, Serialize)]
pub struct DashboardState {
    pub connected: bool,
    pub sync_state: String,
    pub best_bid: Option<f64>,
    pub best_ask: Option<f64>,
    pub spread: Option<f64>,
    pub event_count: u64,
    pub events_per_sec: f64,
    pub bids: Vec<(f64, f64)>,
    pub asks: Vec<(f64, f64)>,
    pub recent_trades: Vec<PaperTrade>,
    pub buy_count: u64,
    pub sell_count: u64,
    pub halted: bool,
    pub price_history: Vec<f64>,
    pub realized_pnl: f64,
    pub return_pct: f64,
    pub position_qty: f64,
    pub avg_entry_price: Option<f64>,
    pub unrealized_pnl: Option<f64>,
}

pub struct SharedState {
    pub connected: AtomicBool,
    pub is_live: AtomicBool,
    pub event_count: AtomicU64,
    pub snapshot: RwLock<Option<OrderbookSnapshot>>,
    pub recent_trades: RwLock<VecDeque<PaperTrade>>,
    pub buy_count: AtomicU64,
    pub sell_count: AtomicU64,
    pub price_history: RwLock<VecDeque<f64>>,
    pub events_last_sec: AtomicU64,
    pub events_last_checkpoint: AtomicU64,
    /// Paper-trading PnL, updated per fill by the execution task (off the
    /// hot path) and read once per second by the dashboard push.
    pub pnl: RwLock<FifoPnl>,
    /// Kill-switch flag shared with the risk checker; kept in sync with the
    /// HALT file by a background poller in main.
    pub halt_flag: Arc<AtomicBool>,
}

impl SharedState {
    pub fn new(halt_flag: Arc<AtomicBool>) -> Arc<Self> {
        Arc::new(Self {
            connected: AtomicBool::new(false),
            is_live: AtomicBool::new(false),
            event_count: AtomicU64::new(0),
            snapshot: RwLock::new(None),
            recent_trades: RwLock::new(VecDeque::with_capacity(50)),
            buy_count: AtomicU64::new(0),
            sell_count: AtomicU64::new(0),
            price_history: RwLock::new(VecDeque::with_capacity(300)),
            events_last_sec: AtomicU64::new(0),
            events_last_checkpoint: AtomicU64::new(0),
            pnl: RwLock::new(FifoPnl::new()),
            halt_flag,
        })
    }

    pub fn add_trade(&self, trade: PaperTrade) {
        let mut trades = self.recent_trades.write();
        if trades.len() >= 50 {
            trades.pop_front();
        }
        trades.push_back(trade);
    }

    pub fn update_price_history(&self, price: f64) {
        let mut history = self.price_history.write();
        if history.len() >= 300 {
            history.pop_front();
        }
        history.push_back(price);
    }

    pub fn to_dashboard_state(&self) -> DashboardState {
        let snap = self.snapshot.read().clone();
        let best_bid = snap.as_ref().and_then(|s| s.bids.first().map(|(p, _)| *p));
        let best_ask = snap.as_ref().and_then(|s| s.asks.first().map(|(p, _)| *p));
        let spread = best_bid.zip(best_ask).map(|(b, a)| a - b);
        let bids = snap.as_ref().map(|s| s.bids.iter().take(10).cloned().collect()).unwrap_or_default();
        let asks = snap.as_ref().map(|s| s.asks.iter().take(10).cloned().collect()).unwrap_or_default();
        let trades: Vec<PaperTrade> = self.recent_trades.read().iter().cloned().collect();
        let price_history: Vec<f64> = self.price_history.read().iter().cloned().collect();
        let halted = self.halt_flag.load(Ordering::Relaxed);

        let (realized_pnl, return_pct, position_qty, avg_entry_price, unrealized_pnl) = {
            let pnl = self.pnl.read();
            (
                pnl.realized_pnl(),
                pnl.return_pct(),
                pnl.position_qty(),
                pnl.avg_entry_price(),
                best_bid.map(|mark| pnl.unrealized_pnl(mark)),
            )
        };

        let event_count = self.event_count.load(Ordering::Relaxed);
        let last_checkpoint = self.events_last_checkpoint.load(Ordering::Relaxed);
        let events_per_sec = (event_count.saturating_sub(last_checkpoint)) as f64;
        self.events_last_checkpoint.store(event_count, Ordering::Relaxed);

        DashboardState {
            connected: self.connected.load(Ordering::Relaxed),
            sync_state: if self.is_live.load(Ordering::Relaxed) { "Live".to_string() } else { "Buffering".to_string() },
            best_bid,
            best_ask,
            spread,
            event_count,
            events_per_sec,
            bids,
            asks,
            recent_trades: trades,
            buy_count: self.buy_count.load(Ordering::Relaxed),
            sell_count: self.sell_count.load(Ordering::Relaxed),
            halted,
            price_history,
            realized_pnl,
            return_pct,
            position_qty,
            avg_entry_price,
            unrealized_pnl,
        }
    }
}
