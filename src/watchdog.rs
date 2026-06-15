// Monitors the last-seen timestamp from ingestion.
// If no message received within timeout_secs, sends Reconnect signal.
use crossbeam_channel::Sender;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{error, info, warn};
use crate::types::MarketEvent;

pub struct Watchdog {
    pub last_seen_ms: Arc<AtomicU64>,
}

impl Watchdog {
    pub fn new() -> Self {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
        Self {
            last_seen_ms: Arc::new(AtomicU64::new(now)),
        }
    }

    pub fn touch(&self) {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
        self.last_seen_ms.store(now, Ordering::Relaxed);
    }

    pub fn run(&self, timeout_secs: u64, tx: Sender<MarketEvent>) {
        let last_seen = Arc::clone(&self.last_seen_ms);
        let timeout_ms = timeout_secs * 1000;
        std::thread::spawn(move || {
            info!("Watchdog started (timeout={}s)", timeout_secs);
            loop {
                std::thread::sleep(Duration::from_secs(5));
                let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
                let last = last_seen.load(Ordering::Relaxed);
                let elapsed_ms = now.saturating_sub(last);
                if elapsed_ms > timeout_ms {
                    warn!(elapsed_ms, "Watchdog: no data received, sending Reconnect");
                    if tx.send(MarketEvent::Reconnect).is_err() {
                        error!("Watchdog: channel closed, exiting");
                        break;
                    }
                }
            }
        });
    }
}
