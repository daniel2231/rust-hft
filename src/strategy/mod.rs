use crate::types::{OrderbookSnapshot, Signal};

/// Strategy implementors must be Send so they can run on the dedicated OS thread.
pub trait Strategy: Send {
    fn on_orderbook(&mut self, snapshot: &OrderbookSnapshot) -> Signal;
}

/// No-op stub used until a real strategy is plugged in.
/// Always returns Hold so no orders are generated.
pub struct NoOpStrategy;

impl Strategy for NoOpStrategy {
    fn on_orderbook(&mut self, _snapshot: &OrderbookSnapshot) -> Signal {
        Signal::Hold
    }
}
