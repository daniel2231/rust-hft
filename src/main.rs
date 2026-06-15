use crypto_trader::config;
use crypto_trader::execution;
use crypto_trader::ingestion;
use crypto_trader::orderbook;
use crypto_trader::risk;
use crypto_trader::strategy;
use crypto_trader::types;
use crypto_trader::watchdog;

use anyhow::Result;
use crossbeam_channel::bounded;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cfg = config::load_config("config/default.toml")?;
    info!(mode = %cfg.mode, symbol = %cfg.symbol, "crypto-trader starting");

    let (market_tx, market_rx) = bounded::<types::MarketEvent>(1024);
    let (order_tx, order_rx) = bounded::<types::ValidatedOrder>(64);

    let cfg_thread = cfg.clone();
    let order_tx_thread = order_tx.clone();
    std::thread::spawn(move || {
        let mut book = orderbook::Orderbook::new(
            cfg_thread.symbol.clone(),
            cfg_thread.orderbook.depth_levels,
        );
        let mut strat: Box<dyn strategy::Strategy> = Box::new(strategy::NoOpStrategy);
        let mut risk = risk::RiskChecker::new(
            cfg_thread.risk.max_order_qty,
            cfg_thread.risk.min_free_balance_usdt,
            cfg_thread.risk.price_band_pct,
            cfg_thread.risk.max_orders_per_sec,
        );
        let mut event_count: u64 = 0;

        for event in &market_rx {
            event_count += 1;
            let queue_len = market_rx.len();
            if queue_len > 100 {
                tracing::warn!(queue_len, "Orderbook thread: channel backlog exceeds 100 items");
            }
            match event {
                types::MarketEvent::DepthUpdate(update) => {
                    match book.handle_update(&update) {
                        Ok(true) => {
                            let snap = book.snapshot();
                            let mid = snap.bids.first().map(|(p, _)| *p).unwrap_or(0.0);
                            let signal = strat.on_orderbook(&snap);

                            // Only call risk.check() for non-Hold signals
                            match &signal {
                                types::Signal::Hold => {}
                                _ => {
                                    match risk.check(&signal, mid) {
                                        Ok(order) => {
                                            let _ = order_tx_thread.send(order);
                                        }
                                        Err(e) => {
                                            tracing::warn!("Risk check rejected order: {}", e);
                                        }
                                    }
                                }
                            }

                            // Every 1000 events, log stats
                            if event_count % 1000 == 0 {
                                let best_bid = snap.bids.first().map(|(p, q)| (*p, *q));
                                let best_ask = snap.asks.first().map(|(p, q)| (*p, *q));
                                info!(
                                    events = event_count,
                                    best_bid = ?best_bid,
                                    best_ask = ?best_ask,
                                    "Orderbook thread: {} events processed",
                                    event_count
                                );
                            }
                        }
                        Ok(false) => {
                            // Buffering or sequence gap — no action needed
                        }
                        Err(e) => {
                            tracing::error!("Orderbook error: {}", e);
                            book.reset();
                        }
                    }
                }
                types::MarketEvent::DepthSnapshot(snapshot) => {
                    book.handle_snapshot(&snapshot);
                }
                types::MarketEvent::Trade(trade) => {
                    tracing::debug!(price = %trade.price, qty = %trade.qty, "Trade");
                }
                types::MarketEvent::Reconnect => {
                    book.reset();
                }
            }
        }
    });

    let executor = Arc::new(execution::PaperExecutor::new(cfg.symbol.clone()));
    tokio::spawn(async move {
        let mut order_count: u64 = 0;
        for order in &order_rx {
            executor.execute(&order);
            order_count += 1;
            if order_count % 100 == 0 {
                executor.log_stats();
            }
        }
    });

    let wd = Arc::new(watchdog::Watchdog::new());
    wd.run(cfg.watchdog.timeout_secs, market_tx.clone());

    let cancel = CancellationToken::new();

    tokio::select! {
        result = ingestion::run_ingestion(
            cfg.exchange.ws_url.clone(),
            cfg.symbol.clone(),
            market_tx,
            Arc::clone(&wd),
            cancel.clone(),
        ) => {
            if let Err(e) = result {
                tracing::error!("Ingestion error: {}", e);
            }
        }
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("Received Ctrl+C, shutting down");
            cancel.cancel();
        }
    }

    Ok(())
}
