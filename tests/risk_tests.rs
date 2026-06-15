/// Risk checker unit tests covering kill switch, qty limits, fat-finger, and valid orders.
use crypto_trader::risk::RiskChecker;
use crypto_trader::types::Signal;

fn make_checker() -> RiskChecker {
    RiskChecker::new(
        0.01,   // max_order_qty
        100.0,  // min_free_balance_usdt
        1.0,    // price_band_pct (1%)
        5,      // max_orders_per_sec
    )
}

/// 1. Kill switch: halted=true must block every order.
#[test]
fn test_kill_switch() {
    let mut checker = make_checker();
    checker.halted = true;

    let signal = Signal::Buy { price: 50000.0, qty: 0.001 };
    let result = checker.check(&signal, 50000.0);
    assert!(result.is_none(), "Order should be blocked when kill switch is active");
}

/// 2. Order qty exceeding max_order_qty must be blocked.
#[test]
fn test_max_order_qty() {
    let mut checker = make_checker();

    // max_order_qty is 0.01; send 0.02
    let signal = Signal::Buy { price: 50000.0, qty: 0.02 };
    let result = checker.check(&signal, 50000.0);
    assert!(result.is_none(), "Order exceeding max qty should be blocked");
}

/// 3. Fat-finger check: price more than price_band_pct away from mid must be blocked.
#[test]
fn test_fat_finger_check() {
    let mut checker = make_checker();
    let mid = 50000.0;

    // price_band_pct = 1%, so a price 2% away should be blocked
    let far_price = mid * 1.02; // 2% above mid
    let signal = Signal::Buy { price: far_price, qty: 0.001 };
    let result = checker.check(&signal, mid);
    assert!(result.is_none(), "Fat-finger price check should block order 2% from mid");
}

/// 4. A valid order (within all limits) must pass all checks and return Some.
#[test]
fn test_valid_order_passes() {
    let mut checker = make_checker();
    checker.free_balance_usdt = 10_000.0;

    let mid = 50000.0;
    // qty within limit, price within 1% band
    let signal = Signal::Buy { price: mid * 1.005, qty: 0.005 };
    let result = checker.check(&signal, mid);
    assert!(result.is_some(), "Valid order should pass all risk checks");

    let order = result.unwrap();
    // client_order_id should be a non-empty UUID string
    assert!(!order.client_order_id.is_empty());
}
