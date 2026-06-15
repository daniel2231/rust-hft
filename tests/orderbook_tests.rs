/// Orderbook unit tests covering the Binance Futures snapshot+diff sync procedure.
///
/// The Orderbook is used directly here. All helpers build DepthUpdate / DepthSnapshot
/// values in memory — no network calls are made.
use crypto_trader::orderbook::Orderbook;
use crypto_trader::types::{DepthSnapshot, DepthUpdate};

fn make_snapshot(last_update_id: u64, bids: Vec<(&str, &str)>, asks: Vec<(&str, &str)>) -> DepthSnapshot {
    DepthSnapshot {
        last_update_id,
        bids: bids.into_iter().map(|(p, q)| [p.to_string(), q.to_string()]).collect(),
        asks: asks.into_iter().map(|(p, q)| [p.to_string(), q.to_string()]).collect(),
    }
}

fn make_update(first_update_id: u64, last_update_id: u64, prev_last_update_id: u64,
               bids: Vec<(&str, &str)>, asks: Vec<(&str, &str)>) -> DepthUpdate {
    DepthUpdate {
        event_type: "depthUpdate".to_string(),
        event_time: 0,
        symbol: "BTCUSDT".to_string(),
        first_update_id,
        last_update_id,
        prev_last_update_id,
        bids: bids.into_iter().map(|(p, q)| [p.to_string(), q.to_string()]).collect(),
        asks: asks.into_iter().map(|(p, q)| [p.to_string(), q.to_string()]).collect(),
    }
}

/// 1. Apply a snapshot, verify bids/asks are present in the snapshot output.
#[test]
fn test_snapshot_apply() {
    let mut book = Orderbook::new("BTCUSDT".to_string(), 10);

    // Buffer an event that satisfies the sync condition for snapshot lastUpdateId=100.
    // U=101 <= S+1=101 AND u=102 >= S+1=101; pu=100 == S so continuity holds.
    let update = make_update(101, 102, 100, vec![], vec![]);
    book.handle_update(&update, &std::sync::atomic::AtomicBool::new(false)).unwrap();

    let snap = make_snapshot(100, vec![("50000.0", "1.5"), ("49999.0", "2.0")],
                                    vec![("50001.0", "0.5")]);
    let synced = book.handle_snapshot(&snap);
    assert!(synced, "Expected orderbook to sync successfully");
    assert!(book.is_live());

    let ob = book.snapshot();
    // Best bid should be 50000.0 (highest)
    assert_eq!(ob.bids[0].0, 50000.0);
    assert_eq!(ob.bids[0].1, 1.5);
    assert_eq!(ob.bids[1].0, 49999.0);
    // Best ask
    assert_eq!(ob.asks[0].0, 50001.0);
    assert_eq!(ob.asks[0].1, 0.5);
}

/// 2. Apply snapshot then a valid diff update; verify state changes.
#[test]
fn test_diff_apply() {
    let mut book = Orderbook::new("BTCUSDT".to_string(), 10);

    // Buffer a diff that satisfies the sync condition for snapshot with lastUpdateId=200.
    // U=201 <= 201, u=202 >= 201, pu=200 == S
    let buf_update = make_update(201, 202, 200, vec![], vec![]);
    book.handle_update(&buf_update, &std::sync::atomic::AtomicBool::new(false)).unwrap();

    let snap = make_snapshot(200, vec![("50000.0", "1.0")], vec![("50001.0", "1.0")]);
    assert!(book.handle_snapshot(&snap));

    // After buffered replay, last_update_id = 202.
    // Next live update: pu=202
    let live_update = make_update(203, 203, 202,
        vec![("49999.0", "3.0")],   // new bid level
        vec![("50002.0", "0.8")]);  // new ask level
    let applied = book.handle_update(&live_update, &std::sync::atomic::AtomicBool::new(false)).unwrap();
    assert!(applied);

    let ob = book.snapshot();
    // Best bid is 50000.0
    assert_eq!(ob.bids[0].0, 50000.0);
    // Second bid is 49999.0 from the diff
    assert_eq!(ob.bids[1].0, 49999.0);
    assert_eq!(ob.bids[1].1, 3.0);
    // Asks: 50001.0 and 50002.0
    assert_eq!(ob.asks[0].0, 50001.0);
    assert_eq!(ob.asks[1].0, 50002.0);
    assert_eq!(ob.asks[1].1, 0.8);
}

/// 3. Sequence gap detection: after going Live, send an update with wrong pu.
///    Orderbook must reset to Buffering.
#[test]
fn test_sequence_gap_detection() {
    let mut book = Orderbook::new("BTCUSDT".to_string(), 10);

    // Buffer + snapshot → Live
    let buf = make_update(301, 302, 300, vec![], vec![]);
    book.handle_update(&buf, &std::sync::atomic::AtomicBool::new(false)).unwrap();
    let snap = make_snapshot(300, vec![("50000.0", "1.0")], vec![("50001.0", "1.0")]);
    assert!(book.handle_snapshot(&snap));
    assert!(book.is_live());

    // Send update with wrong pu (expected 302, send 999)
    let bad_update = make_update(303, 303, 999, vec![], vec![]);
    let result = book.handle_update(&bad_update, &std::sync::atomic::AtomicBool::new(false)).unwrap();
    assert!(!result, "Expected false on sequence gap");
    assert!(!book.is_live(), "Orderbook should have reset to Buffering after gap");
}

/// 4. qty == 0 in a diff update must remove that price level.
#[test]
fn test_qty_zero_removes_level() {
    let mut book = Orderbook::new("BTCUSDT".to_string(), 10);

    // Buffer event satisfying sync condition for snapshot lastUpdateId=400
    let buf = make_update(401, 402, 400, vec![], vec![]);
    book.handle_update(&buf, &std::sync::atomic::AtomicBool::new(false)).unwrap();

    // Snapshot with two bid levels
    let snap = make_snapshot(400, vec![("50000.0", "1.0"), ("49999.0", "2.0")], vec![]);
    assert!(book.handle_snapshot(&snap));

    // Diff update: remove 50000.0 bid (qty=0), pu=402
    let remove_update = make_update(403, 403, 402,
        vec![("50000.0", "0.0")],
        vec![]);
    let applied = book.handle_update(&remove_update, &std::sync::atomic::AtomicBool::new(false)).unwrap();
    assert!(applied);

    let ob = book.snapshot();
    // Only 49999.0 should remain
    assert_eq!(ob.bids.len(), 1);
    assert_eq!(ob.bids[0].0, 49999.0);
}

/// 5. Events sent before the snapshot arrives are buffered and applied after the snapshot.
///    Events outside the valid sync window are discarded.
#[test]
fn test_buffering_state() {
    let mut book = Orderbook::new("BTCUSDT".to_string(), 10);

    // Snapshot will have lastUpdateId=500.
    // Events:
    //   u1: U=498, u=499, pu=497 → u=499 < S+1=501 → will be discarded (before valid window)
    //   u2: U=500, u=501, pu=500 → U=500 <= 501 AND u=501 >= 501 → first valid event; pu==S
    //   u3: U=502, u=503, pu=501 → applied after u2; pu == u2.u
    let u1 = make_update(498, 499, 497, vec![("49900.0", "5.0")], vec![]);
    let u2 = make_update(500, 501, 500, vec![("49950.0", "3.0")], vec![]);
    let u3 = make_update(502, 503, 501, vec![("49980.0", "1.0")], vec![]);

    // All should be buffered, not applied
    assert!(!book.handle_update(&u1, &std::sync::atomic::AtomicBool::new(false)).unwrap());
    assert!(!book.handle_update(&u2, &std::sync::atomic::AtomicBool::new(false)).unwrap());
    assert!(!book.handle_update(&u3, &std::sync::atomic::AtomicBool::new(false)).unwrap());
    assert!(!book.is_live());

    // Send snapshot; first valid buffered event is u2 (u1 is discarded)
    // After sync: snapshot bids + u2 bid + u3 bid applied; u1 bid discarded
    let snap = make_snapshot(500, vec![("50000.0", "2.0")], vec![("50001.0", "0.5")]);
    let synced = book.handle_snapshot(&snap);
    assert!(synced);
    assert!(book.is_live());

    let ob = book.snapshot();
    // Snapshot bid 50000.0 should be present
    assert!(ob.bids.iter().any(|(p, _)| (*p - 50000.0).abs() < 1e-6));
    // u2 added 49950.0, u3 added 49980.0 — both should be present
    assert!(ob.bids.iter().any(|(p, _)| (*p - 49950.0).abs() < 1e-6));
    assert!(ob.bids.iter().any(|(p, _)| (*p - 49980.0).abs() < 1e-6));
    // u1 was discarded (before the valid sync window), so 49900.0 should NOT be present
    assert!(!ob.bids.iter().any(|(p, _)| (*p - 49900.0).abs() < 1e-6));
}
