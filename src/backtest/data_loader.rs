use crate::types::{DepthSnapshot, DepthUpdate};
use anyhow::Result;
use std::fs::File;
use std::io::{BufRead, BufReader};

/// Load historical depth updates from an NDJSON file.
/// Each line must be a valid JSON object deserializable as DepthUpdate.
pub fn load_depth_updates(path: &str) -> Result<Vec<DepthUpdate>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut updates = Vec::new();

    for (line_num, line) in reader.lines().enumerate() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let update: DepthUpdate = serde_json::from_str(trimmed)
            .map_err(|e| anyhow::anyhow!("Line {}: {}", line_num + 1, e))?;
        updates.push(update);
    }

    Ok(updates)
}

/// Generate synthetic depth update data for testing.
/// Deterministic (seeded LCG, no external RNG). The mid price follows a
/// random walk, and each event deletes the previous event's levels (qty 0)
/// before re-quoting around the new mid, so the book tracks the walk and
/// never becomes crossed.
/// Returns (snapshot, events) where snapshot.last_update_id = 1000
/// and events start from there with sequential IDs.
pub fn generate_synthetic_data(symbol: &str, num_events: usize) -> (DepthSnapshot, Vec<DepthUpdate>) {
    let base_price = 65000.0_f64;
    let base_update_id: u64 = 1000;

    // Deterministic LCG (Knuth MMIX constants).
    let mut rng_state: u64 = 0x5DEECE66D;
    let mut next_rand = move || {
        rng_state = rng_state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (rng_state >> 33) as f64 / (1u64 << 31) as f64 // uniform [0, 1)
    };

    // Snapshot: 5 levels each side around the base price.
    let mut snap_bids = Vec::new();
    let mut snap_asks = Vec::new();
    let mut prev_bid_prices: Vec<f64> = Vec::new();
    let mut prev_ask_prices: Vec<f64> = Vec::new();
    for i in 0..5 {
        let bid_price = base_price - (i as f64 + 1.0) * 10.0;
        let ask_price = base_price + (i as f64 + 1.0) * 10.0;
        let qty = 0.25 + next_rand() * 1.5;
        snap_bids.push([format!("{:.2}", bid_price), format!("{:.4}", qty)]);
        snap_asks.push([format!("{:.2}", ask_price), format!("{:.4}", qty)]);
        prev_bid_prices.push(bid_price);
        prev_ask_prices.push(ask_price);
    }

    let snapshot = DepthSnapshot {
        last_update_id: base_update_id,
        bids: snap_bids,
        asks: snap_asks,
    };

    let mut events = Vec::with_capacity(num_events);
    let mut last_id = base_update_id;
    let mut mid = base_price;

    for i in 0..num_events {
        // Random walk: whole-dollar step in [-10, +10] per tick.
        mid += (next_rand() * 20.0 - 10.0).round();

        let new_bid_prices = [mid - 10.0, mid - 20.0];
        let new_ask_prices = [mid + 10.0, mid + 20.0];

        // Delete every previously quoted level that we are not re-quoting,
        // then insert the fresh levels. Deletions come first so a re-used
        // price is not wiped out.
        let mut bids: Vec<[String; 2]> = Vec::new();
        let mut asks: Vec<[String; 2]> = Vec::new();
        for &p in &prev_bid_prices {
            if !new_bid_prices.iter().any(|&np| (np - p).abs() < 0.005) {
                bids.push([format!("{:.2}", p), "0.0000".to_string()]);
            }
        }
        for &p in &prev_ask_prices {
            if !new_ask_prices.iter().any(|&np| (np - p).abs() < 0.005) {
                asks.push([format!("{:.2}", p), "0.0000".to_string()]);
            }
        }
        for &p in &new_bid_prices {
            let qty = 0.25 + next_rand() * 1.5;
            bids.push([format!("{:.2}", p), format!("{:.4}", qty)]);
        }
        for &p in &new_ask_prices {
            let qty = 0.25 + next_rand() * 1.5;
            asks.push([format!("{:.2}", p), format!("{:.4}", qty)]);
        }

        prev_bid_prices.clear();
        prev_bid_prices.extend_from_slice(&new_bid_prices);
        prev_ask_prices.clear();
        prev_ask_prices.extend_from_slice(&new_ask_prices);

        let first_update_id = last_id + 1;
        let last_update_id = last_id + 1;
        let prev_last_update_id = last_id;

        events.push(DepthUpdate {
            event_type: "depthUpdate".to_string(),
            event_time: 1_700_000_000_000 + (i as u64) * 100,
            symbol: symbol.to_string(),
            first_update_id,
            last_update_id,
            prev_last_update_id,
            bids,
            asks,
        });

        last_id = last_update_id;
    }

    (snapshot, events)
}
