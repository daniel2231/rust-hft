use crate::types::{OrderbookSnapshot, Signal};

use super::Strategy;

/// Number of mid-price samples kept for the momentum window. At ~100ms per
/// orderbook event this is ~6.4s of history.
const PRICE_BUF: usize = 64;

/// Momentum scalping strategy.
///
/// Entry (when flat): the mid price has risen more than `entry_mom_pct` over
/// the last `lookback_ticks` samples AND top-of-book volume is skewed to the
/// bid side (`imbalance_min`). Buys at the best ask (taker — momentum is
/// chased, not queued for).
///
/// Exit (when holding), first hit wins:
/// - take profit: best bid ≥ entry × (1 + `tp_pct`)
/// - stop loss:   best bid ≤ entry × (1 − `stop_pct`)
/// - time stop:   held longer than `max_hold_ticks` events
/// All exits sell at the best bid (taker). After an exit the strategy stays
/// flat for `cooldown_ticks` events.
///
/// State is a fixed-size ring buffer plus a few scalars — no allocation per
/// call. Because entries/exits cross the spread, configure taker fees
/// (`[paper] fee_pct = 0.05`) for honest numbers.
pub struct MomentumScalpStrategy {
    qty: f64,
    max_notional: Option<f64>,
    lookback_ticks: usize,
    entry_mom_pct: f64,
    imbalance_min: f64,
    tp_pct: f64,
    stop_pct: f64,
    max_hold_ticks: u32,
    cooldown_ticks: u32,

    // ring buffer of recent mid prices
    prices: [f64; PRICE_BUF],
    head: usize,
    filled: usize,

    holding: bool,
    entry_price: f64,
    entry_qty: f64,
    held_ticks: u32,
    cooldown_left: u32,
}

/// Binance BTCUSDT futures quantity step.
const QTY_STEP: f64 = 0.001;

impl MomentumScalpStrategy {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        qty: f64,
        lookback_ticks: usize,
        entry_mom_pct: f64,
        imbalance_min: f64,
        tp_pct: f64,
        stop_pct: f64,
        max_hold_ticks: u32,
        cooldown_ticks: u32,
    ) -> Self {
        Self {
            qty,
            max_notional: None,
            lookback_ticks: lookback_ticks.min(PRICE_BUF - 1),
            entry_mom_pct,
            imbalance_min,
            tp_pct,
            stop_pct,
            max_hold_ticks,
            cooldown_ticks,
            prices: [0.0; PRICE_BUF],
            head: 0,
            filled: 0,
            holding: false,
            entry_price: 0.0,
            entry_qty: 0.0,
            held_ticks: 0,
            cooldown_left: 0,
        }
    }

    /// Cap entry notional (used to size orders to the virtual capital).
    pub fn with_max_notional(mut self, max_notional: f64) -> Self {
        self.max_notional = Some(max_notional);
        self
    }

    fn push_price(&mut self, mid: f64) {
        self.prices[self.head] = mid;
        self.head = (self.head + 1) % PRICE_BUF;
        if self.filled < PRICE_BUF {
            self.filled += 1;
        }
    }

    /// Mid price `lookback_ticks` samples ago (None until warmed up).
    fn lookback_price(&self) -> Option<f64> {
        if self.filled <= self.lookback_ticks {
            return None;
        }
        let idx = (self.head + PRICE_BUF - 1 - self.lookback_ticks) % PRICE_BUF;
        Some(self.prices[idx])
    }

    fn entry_qty_at(&self, price: f64) -> f64 {
        let mut qty = self.qty;
        if let Some(budget) = self.max_notional {
            if price > 0.0 {
                qty = qty.min(budget / price);
            }
        }
        (qty / QTY_STEP).floor() * QTY_STEP
    }
}

impl Default for MomentumScalpStrategy {
    /// qty 0.01 (matches default risk max_order_qty), momentum +0.05% over
    /// ~3s (30 ticks), bid imbalance ≥ 0.60, take profit +0.08%, stop −0.06%,
    /// time stop 300 ticks (~30s), cooldown 30 ticks (~3s).
    fn default() -> Self {
        Self::new(0.01, 30, 0.05, 0.60, 0.08, 0.06, 300, 30)
    }
}

impl Strategy for MomentumScalpStrategy {
    fn on_orderbook(&mut self, snapshot: &OrderbookSnapshot) -> Signal {
        let (best_bid, best_ask) = match (snapshot.bids.first(), snapshot.asks.first()) {
            (Some((bid, _)), Some((ask, _))) => (*bid, *ask),
            _ => return Signal::Hold,
        };
        let mid = (best_bid + best_ask) / 2.0;
        self.push_price(mid);

        if self.holding {
            self.held_ticks += 1;
            let take_profit = self.entry_price * (1.0 + self.tp_pct / 100.0);
            let stop_out = self.entry_price * (1.0 - self.stop_pct / 100.0);

            let exit_reason = if best_bid >= take_profit {
                Some("take_profit")
            } else if best_bid <= stop_out {
                Some("stop_loss")
            } else if self.held_ticks >= self.max_hold_ticks {
                Some("time_stop")
            } else {
                None
            };

            if let Some(reason) = exit_reason {
                self.holding = false;
                self.cooldown_left = self.cooldown_ticks;
                tracing::info!(
                    reason,
                    entry_price = self.entry_price,
                    exit_price = best_bid,
                    held_ticks = self.held_ticks,
                    "Momentum exit"
                );
                return Signal::Sell { price: best_bid, qty: self.entry_qty };
            }
            return Signal::Hold;
        }

        if self.cooldown_left > 0 {
            self.cooldown_left -= 1;
            return Signal::Hold;
        }

        // Momentum gate: short-term return over the lookback window.
        let momentum_pct = match self.lookback_price() {
            Some(past) if past > 0.0 => (mid - past) / past * 100.0,
            _ => return Signal::Hold, // not warmed up yet
        };
        if momentum_pct < self.entry_mom_pct {
            return Signal::Hold;
        }

        // Confirmation gate: top-5-level volume skewed to the bid side.
        let bid_vol: f64 = snapshot.bids.iter().take(5).map(|(_, q)| q).sum();
        let ask_vol: f64 = snapshot.asks.iter().take(5).map(|(_, q)| q).sum();
        let total = bid_vol + ask_vol;
        if total <= 0.0 || bid_vol / total < self.imbalance_min {
            return Signal::Hold;
        }

        let qty = self.entry_qty_at(best_ask);
        if qty <= 0.0 {
            return Signal::Hold;
        }
        self.holding = true;
        self.entry_price = best_ask;
        self.entry_qty = qty;
        self.held_ticks = 0;
        tracing::info!(
            momentum_pct,
            bid_imbalance = bid_vol / total,
            entry_price = best_ask,
            qty,
            "Momentum entry"
        );
        Signal::Buy { price: best_ask, qty }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(bid: f64, ask: f64, bid_qty: f64, ask_qty: f64) -> OrderbookSnapshot {
        OrderbookSnapshot {
            symbol: "BTCUSDT".to_string(),
            bids: vec![(bid, bid_qty)],
            asks: vec![(ask, ask_qty)],
            timestamp_ms: 0,
        }
    }

    fn warmed_up(strat: &mut MomentumScalpStrategy, price: f64) {
        for _ in 0..40 {
            assert!(matches!(
                strat.on_orderbook(&snap(price - 1.0, price + 1.0, 1.0, 1.0)),
                Signal::Hold
            ));
        }
    }

    #[test]
    fn test_enters_on_momentum_with_bid_imbalance() {
        let mut strat = MomentumScalpStrategy::new(0.01, 10, 0.05, 0.6, 0.08, 0.06, 300, 5);
        warmed_up(&mut strat, 65000.0);

        // Price jumps +0.1% with strong bid volume → Buy at best ask.
        let jumped = 65000.0 * 1.001;
        let sig = strat.on_orderbook(&snap(jumped - 1.0, jumped + 1.0, 3.0, 1.0));
        match sig {
            Signal::Buy { price, qty } => {
                assert!((price - (jumped + 1.0)).abs() < 1e-9);
                assert!(qty > 0.0);
            }
            other => panic!("expected Buy, got {:?}", other),
        }
    }

    #[test]
    fn test_no_entry_without_imbalance() {
        let mut strat = MomentumScalpStrategy::new(0.01, 10, 0.05, 0.6, 0.08, 0.06, 300, 5);
        warmed_up(&mut strat, 65000.0);

        // Same momentum but ask-heavy book → Hold.
        let jumped = 65000.0 * 1.001;
        let sig = strat.on_orderbook(&snap(jumped - 1.0, jumped + 1.0, 1.0, 3.0));
        assert!(matches!(sig, Signal::Hold));
    }

    #[test]
    fn test_take_profit_and_stop_exits() {
        let mut strat = MomentumScalpStrategy::new(0.01, 10, 0.05, 0.6, 0.08, 0.06, 300, 0);
        warmed_up(&mut strat, 65000.0);
        let jumped = 65000.0 * 1.001;
        let entry = match strat.on_orderbook(&snap(jumped - 1.0, jumped + 1.0, 3.0, 1.0)) {
            Signal::Buy { price, .. } => price,
            other => panic!("expected Buy, got {:?}", other),
        };

        // Bid rises past take-profit → Sell.
        let tp_bid = entry * 1.001; // +0.1% > tp 0.08%
        match strat.on_orderbook(&snap(tp_bid, tp_bid + 2.0, 1.0, 1.0)) {
            Signal::Sell { price, .. } => assert!((price - tp_bid).abs() < 1e-9),
            other => panic!("expected Sell, got {:?}", other),
        }
    }

    #[test]
    fn test_time_stop_exits() {
        let mut strat = MomentumScalpStrategy::new(0.01, 10, 0.05, 0.6, 0.08, 0.06, 20, 0);
        warmed_up(&mut strat, 65000.0);
        let jumped = 65000.0 * 1.001;
        assert!(matches!(
            strat.on_orderbook(&snap(jumped - 1.0, jumped + 1.0, 3.0, 1.0)),
            Signal::Buy { .. }
        ));

        // Price goes nowhere; after max_hold_ticks the position is closed.
        let mut sold = false;
        for _ in 0..25 {
            if let Signal::Sell { .. } =
                strat.on_orderbook(&snap(jumped - 1.0, jumped + 1.0, 1.0, 1.0))
            {
                sold = true;
                break;
            }
        }
        assert!(sold, "time stop should have closed the position");
    }
}
