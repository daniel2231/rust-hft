use std::time::Instant;
use tracing::info;

pub struct LatencyTracker {
    label: &'static str,
    start: Instant,
}

impl LatencyTracker {
    pub fn start(label: &'static str) -> Self {
        Self { label, start: Instant::now() }
    }

    pub fn record(self) {
        let elapsed_us = self.start.elapsed().as_micros();
        info!(label = self.label, latency_us = elapsed_us, "Latency");
    }
}
