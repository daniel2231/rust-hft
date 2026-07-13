use std::collections::VecDeque;

/// Incremental FIFO PnL tracker shared by the backtester and the live
/// (paper) execution path.
///
/// Each sell is matched against the oldest open buys:
/// `pnl per match = (sell_price − buy_price) × min(buy_qty, sell_qty)`.
/// Sell quantity with no open buy to match against is ignored.
pub struct FifoPnl {
    buy_queue: VecDeque<(f64, f64)>, // (price, remaining qty)
    realized_pnl: f64,
    buy_notional: f64,
}

const QTY_EPSILON: f64 = 1e-10;

impl FifoPnl {
    pub fn new() -> Self {
        Self {
            buy_queue: VecDeque::new(),
            realized_pnl: 0.0,
            buy_notional: 0.0,
        }
    }

    pub fn on_buy(&mut self, price: f64, qty: f64) {
        self.buy_queue.push_back((price, qty));
        self.buy_notional += price * qty;
    }

    pub fn on_sell(&mut self, price: f64, qty: f64) {
        let mut remaining = qty;
        while remaining > QTY_EPSILON {
            match self.buy_queue.front_mut() {
                Some((buy_price, buy_qty)) => {
                    let matched = remaining.min(*buy_qty);
                    self.realized_pnl += (price - *buy_price) * matched;
                    *buy_qty -= matched;
                    remaining -= matched;
                    if *buy_qty <= QTY_EPSILON {
                        self.buy_queue.pop_front();
                    }
                }
                None => break,
            }
        }
    }

    pub fn realized_pnl(&self) -> f64 {
        self.realized_pnl
    }

    /// Total notional of all buys (matched or not) — the return denominator.
    pub fn buy_notional(&self) -> f64 {
        self.buy_notional
    }

    /// Realized PnL as a percentage of total buy notional.
    pub fn return_pct(&self) -> f64 {
        if self.buy_notional > 0.0 {
            self.realized_pnl / self.buy_notional * 100.0
        } else {
            0.0
        }
    }

    /// Open (not yet sold) position size.
    pub fn position_qty(&self) -> f64 {
        self.buy_queue.iter().map(|(_, q)| q).sum()
    }

    /// Volume-weighted average entry price of the open position.
    pub fn avg_entry_price(&self) -> Option<f64> {
        let qty = self.position_qty();
        if qty > QTY_EPSILON {
            let notional: f64 = self.buy_queue.iter().map(|(p, q)| p * q).sum();
            Some(notional / qty)
        } else {
            None
        }
    }

    /// Mark-to-market PnL of the open position at `mark_price`.
    pub fn unrealized_pnl(&self, mark_price: f64) -> f64 {
        self.buy_queue
            .iter()
            .map(|(p, q)| (mark_price - p) * q)
            .sum()
    }
}

impl Default for FifoPnl {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_round_trip() {
        let mut pnl = FifoPnl::new();
        pnl.on_buy(100.0, 1.0);
        pnl.on_sell(110.0, 1.0);
        assert!((pnl.realized_pnl() - 10.0).abs() < 1e-9);
        assert!((pnl.return_pct() - 10.0).abs() < 1e-9);
        assert!(pnl.position_qty() < 1e-9);
        assert_eq!(pnl.avg_entry_price(), None);
    }

    #[test]
    fn test_fifo_order_and_partial_fill() {
        let mut pnl = FifoPnl::new();
        pnl.on_buy(100.0, 1.0);
        pnl.on_buy(200.0, 1.0);
        // Sell 1.5: matches the 100 buy fully, half of the 200 buy.
        pnl.on_sell(150.0, 1.5);
        // (150-100)*1.0 + (150-200)*0.5 = 50 - 25 = 25
        assert!((pnl.realized_pnl() - 25.0).abs() < 1e-9);
        assert!((pnl.position_qty() - 0.5).abs() < 1e-9);
        assert!((pnl.avg_entry_price().unwrap() - 200.0).abs() < 1e-9);
    }

    #[test]
    fn test_unmatched_sell_is_ignored() {
        let mut pnl = FifoPnl::new();
        pnl.on_sell(100.0, 1.0);
        assert_eq!(pnl.realized_pnl(), 0.0);
        pnl.on_buy(100.0, 1.0);
        pnl.on_sell(105.0, 2.0); // only 1.0 matches
        assert!((pnl.realized_pnl() - 5.0).abs() < 1e-9);
        assert!(pnl.position_qty() < 1e-9);
    }

    #[test]
    fn test_unrealized_pnl() {
        let mut pnl = FifoPnl::new();
        pnl.on_buy(100.0, 2.0);
        assert!((pnl.unrealized_pnl(110.0) - 20.0).abs() < 1e-9);
        assert!((pnl.unrealized_pnl(90.0) + 20.0).abs() < 1e-9);
    }
}
