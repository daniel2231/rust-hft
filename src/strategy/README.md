# Strategy Implementation Guide

To implement a custom strategy:

1. Create a new file in this directory, e.g. `src/strategy/my_strategy.rs`
2. Implement the `Strategy` trait:

```rust
use crate::types::{OrderbookSnapshot, Signal};
use super::Strategy;

pub struct MyStrategy {
    // your state here
}

impl Strategy for MyStrategy {
    fn on_orderbook(&mut self, snapshot: &OrderbookSnapshot) -> Signal {
        // your logic here
        Signal::Hold
    }
}
```

3. In `src/main.rs`, replace `NoOpStrategy` with your implementation.

## Contract

- `on_orderbook` is called on every orderbook update from a dedicated OS thread (not async)
- Return `Signal::Hold` to do nothing
- Return `Signal::Buy` or `Signal::Sell` with price and qty to trigger the risk + execution pipeline
- Do NOT block this function — keep it fast (microseconds)
