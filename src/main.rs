mod config;
mod execution;
mod ingestion;
mod metrics;
mod orderbook;
mod risk;
mod strategy;
mod types;

use anyhow::Result;
use crossbeam_channel::bounded;
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

        for event in &market_rx {
            match event {
                types::MarketEvent::DepthUpdate(update) => {
                    match book.apply_update(&update) {
                        Ok(true) => {
                            let snap = book.snapshot();
                            let mid = snap.bids.first().map(|(p, _)| *p).unwrap_or(0.0);
                            let signal = strat.on_orderbook(&snap);
                            if let Some(order) = risk.check(&signal, mid) {
                                let _ = order_tx_thread.send(order);
                            }
                        }
                        Ok(false) => {
                            book.reset();
                        }
                        Err(e) => {
                            tracing::error!("Orderbook error: {}", e);
                            book.reset();
                        }
                    }
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

    let executor = execution::PaperExecutor::new(cfg.symbol.clone());
    tokio::spawn(async move {
        for order in &order_rx {
            executor.execute(&order);
        }
    });

    ingestion::run_ingestion(cfg.exchange.ws_url.clone(), cfg.symbol.clone(), market_tx).await?;

    Ok(())
}
