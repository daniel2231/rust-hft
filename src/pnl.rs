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

    /// Returns the quantity actually matched against open buys — sell
    /// quantity beyond the open position is ignored (no shorting).
    pub fn on_sell(&mut self, price: f64, qty: f64) -> f64 {
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
        qty - remaining
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

    /// Cost basis of the open position (Σ entry price × remaining qty).
    pub fn position_notional(&self) -> f64 {
        self.buy_queue.iter().map(|(p, q)| p * q).sum()
    }
}

/// Simulated trading account for paper mode: a cash balance funded with a
/// configurable initial capital, debited/credited per fill with a
/// configurable fee. Wraps [`FifoPnl`] for position/PnL accounting.
pub struct PaperAccount {
    pnl: FifoPnl,
    initial_capital: f64,
    cash: f64,
    fee_pct: f64,
    total_fees: f64,
}

impl PaperAccount {
    pub fn new(initial_capital: f64, fee_pct: f64) -> Self {
        Self {
            pnl: FifoPnl::new(),
            initial_capital,
            cash: initial_capital,
            fee_pct,
            total_fees: 0.0,
        }
    }

    pub fn on_buy(&mut self, price: f64, qty: f64) {
        let notional = price * qty;
        let fee = notional * self.fee_pct / 100.0;
        self.pnl.on_buy(price, qty);
        self.cash -= notional + fee;
        self.total_fees += fee;
    }

    /// Credits cash only for the quantity actually matched against the open
    /// position, so an unmatched sell can't mint money out of nothing.
    pub fn on_sell(&mut self, price: f64, qty: f64) {
        let matched = self.pnl.on_sell(price, qty);
        if matched <= QTY_EPSILON {
            return;
        }
        let notional = price * matched;
        let fee = notional * self.fee_pct / 100.0;
        self.cash += notional - fee;
        self.total_fees += fee;
    }

    pub fn pnl(&self) -> &FifoPnl {
        &self.pnl
    }

    pub fn initial_capital(&self) -> f64 {
        self.initial_capital
    }

    pub fn cash(&self) -> f64 {
        self.cash
    }

    pub fn total_fees(&self) -> f64 {
        self.total_fees
    }

    /// Cash plus the open position valued at `mark_price` (falls back to the
    /// position's cost basis when no market price is available).
    pub fn equity(&self, mark_price: Option<f64>) -> f64 {
        let position_value = match mark_price {
            Some(m) => self.pnl.position_qty() * m,
            None => self.pnl.position_notional(),
        };
        self.cash + position_value
    }

    /// Total return vs the initial capital, fees included.
    pub fn return_on_capital_pct(&self, mark_price: Option<f64>) -> f64 {
        if self.initial_capital > 0.0 {
            (self.equity(mark_price) - self.initial_capital) / self.initial_capital * 100.0
        } else {
            0.0
        }
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

    #[test]
    fn test_account_round_trip_with_fees() {
        let mut acct = PaperAccount::new(730.0, 0.02);
        acct.on_buy(100.0, 1.0); // cost 100 + 0.02 fee
        assert!((acct.cash() - 629.98).abs() < 1e-9);
        acct.on_sell(110.0, 1.0); // proceeds 110 - 0.022 fee
        assert!((acct.cash() - 739.958).abs() < 1e-9);
        assert!((acct.total_fees() - 0.042).abs() < 1e-9);
        // flat position: equity == cash, return vs capital includes fees
        assert!((acct.equity(Some(105.0)) - acct.cash()).abs() < 1e-9);
        assert!((acct.return_on_capital_pct(None) - (739.958 - 730.0) / 730.0 * 100.0).abs() < 1e-9);
    }

    #[test]
    fn test_account_equity_marks_open_position() {
        let mut acct = PaperAccount::new(1000.0, 0.0);
        acct.on_buy(100.0, 2.0); // cash 800, position 2.0
        assert!((acct.equity(Some(110.0)) - 1020.0).abs() < 1e-9);
        assert!((acct.equity(None) - 1000.0).abs() < 1e-9); // cost-basis fallback
        assert!((acct.return_on_capital_pct(Some(110.0)) - 2.0).abs() < 1e-9);
    }

    #[test]
    fn test_account_unmatched_sell_credits_nothing() {
        let mut acct = PaperAccount::new(500.0, 0.02);
        acct.on_sell(100.0, 1.0); // no position — must not mint cash
        assert!((acct.cash() - 500.0).abs() < 1e-9);
        assert_eq!(acct.total_fees(), 0.0);
    }
}
