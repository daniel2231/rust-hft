use crypto_trader::config;
use crypto_trader::dashboard;
use crypto_trader::execution;
use crypto_trader::ingestion;
use crypto_trader::orderbook;
use crypto_trader::risk;
use crypto_trader::strategy;
use crypto_trader::types;
use crypto_trader::watchdog;

use anyhow::Result;
use crossbeam_channel::bounded;
use std::sync::atomic::{AtomicBool, Ordering};
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
    info!(mode = %cfg.mode, symbol = %cfg.symbol, strategy = %cfg.strategy, "crypto-trader starting");

    let (market_tx, market_rx) = bounded::<types::MarketEvent>(1024);
    let (order_tx, order_rx) = bounded::<types::ValidatedOrder>(64);

    let resync_needed = Arc::new(AtomicBool::new(false));

    // Kill-switch flag: a background task polls the HALT file so the hot path
    // (risk checks) reads an atomic instead of doing a filesystem stat per order.
    let halt_flag = Arc::new(AtomicBool::new(std::path::Path::new("HALT").exists()));
    {
        let flag = Arc::clone(&halt_flag);
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(std::time::Duration::from_millis(500));
            loop {
                tick.tick().await;
                flag.store(std::path::Path::new("HALT").exists(), Ordering::Relaxed);
            }
        });
    }

    // Shared dashboard state
    let dash_state = dashboard::SharedState::new(
        cfg.symbol.clone(),
        cfg.paper.initial_capital_usdt,
        cfg.paper.fee_pct,
        Arc::clone(&halt_flag),
    );

    let cfg_thread = cfg.clone();
    let order_tx_thread = order_tx.clone();
    let resync_thread = Arc::clone(&resync_needed);
    let dash_state_ob = Arc::clone(&dash_state);
    let halt_flag_risk = Arc::clone(&halt_flag);
    std::thread::spawn(move || {
        let mut book = orderbook::Orderbook::new(
            cfg_thread.symbol.clone(),
            cfg_thread.orderbook.depth_levels,
        );
        // Entry sizes are capped at ~90% of the virtual capital so the
        // strategies trade within the configured account.
        let strategy_budget = cfg_thread.paper.initial_capital_usdt * 0.9;
        let mut strat: Box<dyn strategy::Strategy> = match cfg_thread.strategy.as_str() {
            "momentum" => Box::new(
                strategy::MomentumScalpStrategy::default().with_max_notional(strategy_budget),
            ),
            "pingpong" => Box::new(
                strategy::PingPongStrategy::default().with_max_notional(strategy_budget),
            ),
            "noop" => Box::new(strategy::NoOpStrategy),
            other => {
                tracing::warn!(strategy = other, "Unknown strategy in config — falling back to noop");
                Box::new(strategy::NoOpStrategy)
            }
        };
        let mut risk = risk::RiskChecker::new(
            cfg_thread.risk.max_order_qty,
            cfg_thread.risk.min_free_balance_usdt,
            cfg_thread.risk.price_band_pct,
            cfg_thread.risk.max_orders_per_sec,
        )
        .with_halt_flag(halt_flag_risk)
        .with_balance_source(Arc::clone(&dash_state_ob.cash_bits));
        let mut event_count: u64 = 0;

        for event in &market_rx {
            event_count += 1;
            let queue_len = market_rx.len();
            if queue_len > 100 {
                tracing::warn!(queue_len, "Orderbook thread: channel backlog exceeds 100 items");
            }
            match event {
                types::MarketEvent::DepthUpdate(update) => {
                    match book.handle_update(&update, &resync_thread) {
                        Ok(true) => {
                            // Update shared dashboard state
                            dash_state_ob.event_count.fetch_add(1, Ordering::Relaxed);
                            dash_state_ob.is_live.store(book.is_live(), Ordering::Relaxed);
                            dash_state_ob.connected.store(true, Ordering::Relaxed);
                            if book.is_live() {
                                let snap = book.snapshot();
                                if let Some((price, _)) = snap.bids.first() {
                                    dash_state_ob.update_price_history(*price);
                                }

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

                                // Strategy/risk are done with the snapshot — move it
                                // into the dashboard slot instead of cloning it.
                                *dash_state_ob.snapshot.write() = Some(snap);
                            }

                            // Every 1000 events, log stats
                            if event_count % 1000 == 0 {
                                let snap = book.snapshot();
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
                            // Buffering or sequence gap — update connected state
                            dash_state_ob.event_count.fetch_add(1, Ordering::Relaxed);
                            dash_state_ob.connected.store(true, Ordering::Relaxed);
                            dash_state_ob.is_live.store(false, Ordering::Relaxed);
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
                    dash_state_ob.connected.store(false, Ordering::Relaxed);
                    dash_state_ob.is_live.store(false, Ordering::Relaxed);
                }
            }
        }
    });

    let executor = Arc::new(execution::PaperExecutor::new(cfg.symbol.clone()));
    let dash_state_exec = Arc::clone(&dash_state);
    tokio::spawn(async move {
        let mut order_count: u64 = 0;
        for order in &order_rx {
            executor.execute_with_state(&order, &dash_state_exec);
            order_count += 1;
            if order_count % 100 == 0 {
                executor.log_stats();
            }
        }
    });

    let wd = Arc::new(watchdog::Watchdog::new());
    wd.run(cfg.watchdog.timeout_secs, market_tx.clone());

    // Spawn dashboard server
    if cfg.dashboard.enabled {
        let dash_state_server = Arc::clone(&dash_state);
        let port = cfg.dashboard.port;
        tokio::spawn(async move {
            dashboard::server::run_dashboard(dash_state_server, port).await;
        });
    }

    let cancel = CancellationToken::new();

    tokio::select! {
        result = ingestion::run_ingestion(
            cfg.exchange.ws_url.clone(),
            cfg.symbol.clone(),
            market_tx,
            Arc::clone(&wd),
            cancel.clone(),
            Arc::clone(&resync_needed),
        ) => {
            if let Err(e) = result {
                tracing::error!("Ingestion error: {}", e);
            }
        }
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("Received Ctrl+C, shutting down");
            cancel.cancel();
            std::process::exit(0);
        }
    }

    Ok(())
}
