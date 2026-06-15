pub mod data_loader;

use crate::orderbook::Orderbook;
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
            match self.orderbook.handle_update(&event) {
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

        let realized_pnl = compute_fifo_pnl(&trades);
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

/// FIFO PnL matching: each Buy is matched with the next Sell.
/// PnL per pair = (sell_price - buy_price) * min(buy_qty, sell_qty)
fn compute_fifo_pnl(trades: &[Trade]) -> f64 {
    let mut buy_queue: std::collections::VecDeque<(f64, f64)> = std::collections::VecDeque::new();
    let mut pnl = 0.0;

    for trade in trades {
        match trade.side {
            TradeSide::Buy => {
                buy_queue.push_back((trade.price, trade.qty));
            }
            TradeSide::Sell => {
                let mut remaining_sell_qty = trade.qty;
                while remaining_sell_qty > 1e-10 {
                    if let Some((buy_price, buy_qty)) = buy_queue.front_mut() {
                        let matched = remaining_sell_qty.min(*buy_qty);
                        pnl += (trade.price - *buy_price) * matched;
                        *buy_qty -= matched;
                        remaining_sell_qty -= matched;
                        if *buy_qty <= 1e-10 {
                            buy_queue.pop_front();
                        }
                    } else {
                        break;
                    }
                }
            }
        }
    }

    pnl
}
