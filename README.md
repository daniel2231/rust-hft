# crypto-trader

Low-latency cryptocurrency algorithmic trading system written in Rust.

## Architecture

Multi-threaded pipeline designed for minimal GC latency:

```
[Binance WebSocket] → ingestion → orderbook → strategy → risk → execution
```

- **Thread 1 (Tokio async):** WebSocket ingestion, REST snapshot fetch, order execution
- **Thread 2 (OS thread):** Orderbook maintenance, strategy evaluation, risk checks
- **Bridge:** `crossbeam-channel` between async and sync worlds

## Milestones

| Milestone | Status | Description |
|-----------|--------|-------------|
| M1 | ✅ | Project setup, WebSocket connection, raw stream output |
| M2 | ✅ | Orderbook sync (Snapshot+Diff), unit tests |
| M3 | ✅ | Multi-thread pipeline, crossbeam channels, watchdog, graceful shutdown |
| M4 | ✅ | Risk checks + paper trading mode |
| M5 | 🔲 | Backtesting harness |
| M6 | 🔲 | Strategy implementation, cloud VM deployment |

## Quick Start

### Prerequisites
- Rust 1.82+
- (Optional) Binance API key for live mode

### Run (paper mode)

```bash
cp .env.example .env
# Edit .env with your API keys (not required for paper mode)
RUST_LOG=info cargo run
```

### Configuration

Edit `config/default.toml`:

```toml
mode = "paper"   # "paper" or "live"
symbol = "BTCUSDT"

[risk]
max_order_qty = 0.01
min_free_balance_usdt = 100.0
price_band_pct = 1.0
max_orders_per_sec = 5
```

### Tests

```bash
cargo test
```

## Strategy Interface

Implement the `Strategy` trait in `src/strategy/`:

```rust
pub trait Strategy: Send {
    fn on_orderbook(&mut self, snapshot: &OrderbookSnapshot) -> Signal;
}
```

See `src/strategy/README.md` for details.

## Security


- API keys are loaded from environment variables only — never commit `.env`
- Paper mode makes no real orders

## Phase Roadmap

- **Phase 1 (current):** Mac Mini local development, RTT ~40–80ms, correctness focus
- **Phase 2:** Cloud VM (AWS ap-northeast-1), RTT ~2–5ms, live trading
