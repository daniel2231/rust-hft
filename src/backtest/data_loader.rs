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
/// Uses a deterministic pattern (no external RNG).
/// Returns (snapshot, events) where snapshot.last_update_id = 1000
/// and events start from there with sequential IDs.
pub fn generate_synthetic_data(symbol: &str, num_events: usize) -> (DepthSnapshot, Vec<DepthUpdate>) {
    // Fixed price offsets to cycle through (deterministic variation)
    let price_deltas: [f64; 8] = [0.0, 10.0, -5.0, 15.0, -10.0, 5.0, -15.0, 20.0];
    let qty_values: [f64; 4] = [0.5, 1.0, 0.25, 0.75];

    let base_price = 65000.0_f64;
    let base_update_id: u64 = 1000;

    // Build snapshot bids/asks
    let mut snap_bids = Vec::new();
    let mut snap_asks = Vec::new();
    for i in 0..5 {
        let bid_price = base_price - (i as f64 + 1.0) * 10.0;
        let ask_price = base_price + (i as f64 + 1.0) * 10.0;
        let qty = qty_values[i % qty_values.len()];
        snap_bids.push([format!("{:.2}", bid_price), format!("{:.4}", qty)]);
        snap_asks.push([format!("{:.2}", ask_price), format!("{:.4}", qty)]);
    }

    let snapshot = DepthSnapshot {
        last_update_id: base_update_id,
        bids: snap_bids,
        asks: snap_asks,
    };

    // Generate sequential depth update events
    let mut events = Vec::with_capacity(num_events);
    let mut last_id = base_update_id;

    for i in 0..num_events {
        let delta = price_deltas[i % price_deltas.len()];
        let mid = base_price + delta;
        let qty = qty_values[i % qty_values.len()];

        let first_update_id = last_id + 1;
        let last_update_id = last_id + 1;
        let prev_last_update_id = last_id;

        let bids = vec![
            [format!("{:.2}", mid - 10.0), format!("{:.4}", qty)],
            [format!("{:.2}", mid - 20.0), format!("{:.4}", qty * 2.0)],
        ];
        let asks = vec![
            [format!("{:.2}", mid + 10.0), format!("{:.4}", qty)],
            [format!("{:.2}", mid + 20.0), format!("{:.4}", qty * 2.0)],
        ];

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
