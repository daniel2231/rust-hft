pub mod data_loader;

use crate::orderbook::Orderbook;
use crate::pnl::FifoPnl;
use crate::risk::RiskChecker;
use crate::strategy::Strategy;
use crate::types::{DepthSnapshot, DepthUpdate, Signal};

#[derive(Debug, Clone)]
pub enum TradeSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone)]
pub struct Trade {
    pub side: TradeSide,
    pub price: f64,
    pub qty: f64,
    pub timestamp_ms: u64,
}

#[derive(Debug)]
pub struct BacktestResult {
    pub total_trades: usize,
    pub buy_trades: usize,
    pub sell_trades: usize,
    pub total_volume: f64,
    pub realized_pnl: f64,
    pub trades: Vec<Trade>,
}

pub struct Backtester {
    orderbook: Orderbook,
    strategy: Box<dyn Strategy>,
    risk: RiskChecker,
}

impl Backtester {
    pub fn new(
        symbol: String,
        depth_levels: usize,
        strategy: Box<dyn Strategy>,
        risk: RiskChecker,
    ) -> Self {
        Self {
            orderbook: Orderbook::new(symbol, depth_levels),
            strategy,
            risk,
        }
    }

    pub fn run(&mut self, events: Vec<DepthUpdate>, snapshot: Option<DepthSnapshot>) -> BacktestResult {
        // Apply snapshot first if provided
        if let Some(snap) = snapshot {
            self.orderbook.handle_snapshot(&snap);
        }

        let mut trades: Vec<Trade> = Vec::new();

        for event in events {
            let timestamp_ms = event.event_time;
            match self.orderbook.handle_update(&event, &std::sync::atomic::AtomicBool::new(false)) {
                Ok(true) => {
                    let snap = self.orderbook.snapshot();
                    let mid = snap.bids.first().map(|(p, _)| *p).unwrap_or(0.0);
                    let signal = self.strategy.on_orderbook(&snap);

                    match &signal {
                        Signal::Hold => {}
                        _ => {
                            match self.risk.check(&signal, mid) {
                                Ok(validated) => {
                                    match &validated.signal {
                                        Signal::Buy { price, qty } => {
                                            trades.push(Trade {
                                                side: TradeSide::Buy,
                                                price: *price,
                                                qty: *qty,
                                                timestamp_ms,
                                            });
                                        }
                                        Signal::Sell { price, qty } => {
                                            trades.push(Trade {
                                                side: TradeSide::Sell,
                                                price: *price,
                                                qty: *qty,
                                                timestamp_ms,
                                            });
                                        }
                                        Signal::Hold => {}
                                    }
                                }
                                Err(_) => {}
                            }
                        }
                    }
                }
                Ok(false) => {}
                Err(_) => {}
            }
        }

        let mut pnl = FifoPnl::new();
        for trade in &trades {
            match trade.side {
                TradeSide::Buy => pnl.on_buy(trade.price, trade.qty, trade.timestamp_ms),
                TradeSide::Sell => {
                    pnl.on_sell(trade.price, trade.qty);
                }
            }
        }
        let realized_pnl = pnl.realized_pnl();
        let buy_trades = trades.iter().filter(|t| matches!(t.side, TradeSide::Buy)).count();
        let sell_trades = trades.iter().filter(|t| matches!(t.side, TradeSide::Sell)).count();
        let total_volume: f64 = trades.iter().map(|t| t.qty).sum();
        let total_trades = trades.len();

        BacktestResult {
            total_trades,
            buy_trades,
            sell_trades,
            total_volume,
            realized_pnl,
            trades,
        }
    }
}
