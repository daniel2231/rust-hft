use crypto_trader::backtest::data_loader;
use crypto_trader::backtest::{Backtester, TradeSide};
use crypto_trader::risk::RiskChecker;
use crypto_trader::strategy::Strategy;
use crypto_trader::types::{OrderbookSnapshot, Signal};

#[test]
fn test_synthetic_data_generation() {
    let num_events = 50;
    let (snapshot, events) = data_loader::generate_synthetic_data("BTCUSDT", num_events);

    assert_eq!(events.len(), num_events);
    assert_eq!(snapshot.last_update_id, 1000);
    assert!(!snapshot.bids.is_empty());
    assert!(!snapshot.asks.is_empty());

    // Verify sequential IDs
    let mut last_id = snapshot.last_update_id;
    for event in &events {
        assert_eq!(event.prev_last_update_id, last_id, "prev_last_update_id should match prior last_update_id");
        assert_eq!(event.first_update_id, last_id + 1);
        last_id = event.last_update_id;
    }
}

#[test]
fn test_backtester_noop_strategy() {
    let (snapshot, events) = data_loader::generate_synthetic_data("BTCUSDT", 20);

    let strategy = Box::new(crypto_trader::strategy::NoOpStrategy);
    let risk = RiskChecker::new(1.0, 100.0, 10.0, 10);

    let mut backtester = Backtester::new("BTCUSDT".to_string(), 20, strategy, risk);
    let result = backtester.run(events, Some(snapshot));

    assert_eq!(result.total_trades, 0);
    assert_eq!(result.buy_trades, 0);
    assert_eq!(result.sell_trades, 0);
    assert_eq!(result.total_volume, 0.0);
    assert_eq!(result.realized_pnl, 0.0);
}

// A strategy that alternates: Buy on call 0, Hold on 1..N, Sell on N+1, etc.
struct AlternateBuyStrategy {
    call_count: usize,
    buy_price: f64,
    sell_price: f64,
    buy_qty: f64,
    // How many holds between buy and sell
    hold_gap: usize,
}

impl AlternateBuyStrategy {
    fn new(buy_price: f64, sell_price: f64, qty: f64, hold_gap: usize) -> Self {
        Self { call_count: 0, buy_price, sell_price, buy_qty: qty, hold_gap }
    }
}

impl Strategy for AlternateBuyStrategy {
    fn on_orderbook(&mut self, _snapshot: &OrderbookSnapshot) -> Signal {
        let cycle_len = self.hold_gap + 2; // buy + hold_gap holds + sell
        let pos = self.call_count % cycle_len;
        self.call_count += 1;

        if pos == 0 {
            Signal::Buy { price: self.buy_price, qty: self.buy_qty }
        } else if pos == self.hold_gap + 1 {
            Signal::Sell { price: self.sell_price, qty: self.buy_qty }
        } else {
            Signal::Hold
        }
    }
}

#[test]
fn test_pnl_calculation() {
    // Use synthetic data with enough events for at least 2 buy/sell cycles
    let (snapshot, events) = data_loader::generate_synthetic_data("BTCUSDT", 30);

    let buy_price = 65000.0_f64;
    let sell_price = 65100.0_f64;
    let qty = 0.01_f64;
    let hold_gap = 2;

    // Strategy alternates: Buy, Hold, Hold, Sell, Buy, Hold, Hold, Sell, ...
    let strategy = Box::new(AlternateBuyStrategy::new(buy_price, sell_price, qty, hold_gap));

    // Use permissive risk: high qty, no price band constraint (set price_band_pct large)
    let risk = RiskChecker::new(
        10.0,    // max_order_qty — large
        0.0,     // min_free_balance_usdt — no balance constraint
        50.0,    // price_band_pct — very wide band
        100,     // max_orders_per_sec — high
    );

    let mut backtester = Backtester::new("BTCUSDT".to_string(), 20, strategy, risk);
    let result = backtester.run(events, Some(snapshot));

    // We should have matched buy/sell pairs
    assert!(result.buy_trades > 0, "Expected buy trades");
    assert!(result.sell_trades > 0, "Expected sell trades");

    let matched_pairs = result.buy_trades.min(result.sell_trades);
    let expected_pnl_per_pair = (sell_price - buy_price) * qty;
    let expected_pnl = expected_pnl_per_pair * matched_pairs as f64;

    // Allow small floating point tolerance
    let diff = (result.realized_pnl - expected_pnl).abs();
    assert!(
        diff < 1e-6,
        "Expected PnL ~{:.6} but got {:.6} (diff={:.6})",
        expected_pnl, result.realized_pnl, diff
    );
}
