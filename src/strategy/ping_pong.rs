use crate::types::{OrderbookSnapshot, Signal};

use super::Strategy;

/// Simple ping-pong (spread-capture) test strategy.
///
/// When flat: buy `qty` at the current best bid (assumes a passive fill).
/// When holding: sell at the current best ask once it clears the entry price
/// by `min_edge_pct`, or bail out at the best bid if it drops `stop_pct`
/// below entry.
///
/// This is a TESTING strategy: it assumes every passive order fills at its
/// quoted price and ignores fees/queue position, so backtest PnL is an
/// optimistic upper bound. All state is a few floats — no allocation per call.
pub struct PingPongStrategy {
    qty: f64,
    min_edge_pct: f64,
    stop_pct: f64,
    holding: bool,
    entry_price: f64,
}

impl PingPongStrategy {
    pub fn new(qty: f64, min_edge_pct: f64, stop_pct: f64) -> Self {
        Self {
            qty,
            min_edge_pct,
            stop_pct,
            holding: false,
            entry_price: 0.0,
        }
    }
}

impl Default for PingPongStrategy {
    /// qty 0.01 (matches default risk max_order_qty), 0.02% take-profit edge,
    /// 0.5% stop-out.
    fn default() -> Self {
        Self::new(0.01, 0.02, 0.5)
    }
}

impl Strategy for PingPongStrategy {
    fn on_orderbook(&mut self, snapshot: &OrderbookSnapshot) -> Signal {
        let (best_bid, best_ask) = match (snapshot.bids.first(), snapshot.asks.first()) {
            (Some((bid, _)), Some((ask, _))) => (*bid, *ask),
            _ => return Signal::Hold,
        };

        if !self.holding {
            self.holding = true;
            self.entry_price = best_bid;
            return Signal::Buy { price: best_bid, qty: self.qty };
        }

        let take_profit = self.entry_price * (1.0 + self.min_edge_pct / 100.0);
        let stop_out = self.entry_price * (1.0 - self.stop_pct / 100.0);

        if best_ask >= take_profit {
            self.holding = false;
            return Signal::Sell { price: best_ask, qty: self.qty };
        }
        if best_bid <= stop_out {
            self.holding = false;
            return Signal::Sell { price: best_bid, qty: self.qty };
        }

        Signal::Hold
    }
}
