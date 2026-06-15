use anyhow::Result;
use crypto_trader::backtest::data_loader;
use crypto_trader::backtest::Backtester;
use crypto_trader::config;
use crypto_trader::risk::RiskChecker;
use crypto_trader::strategy::NoOpStrategy;

fn main() -> Result<()> {
    let cfg = config::load_config("config/default.toml")?;

    let args: Vec<String> = std::env::args().collect();
    let data_path = args.get(1).map(|s| s.as_str());

    let (snapshot, events) = match data_path {
        Some(path) => {
            let events = data_loader::load_depth_updates(path)?;
            (None, events)
        }
        None => {
            let (snap, evts) = data_loader::generate_synthetic_data(&cfg.symbol, 100);
            (Some(snap), evts)
        }
    };

    let strategy = Box::new(NoOpStrategy);
    let risk = RiskChecker::new(
        cfg.risk.max_order_qty,
        cfg.risk.min_free_balance_usdt,
        cfg.risk.price_band_pct,
        cfg.risk.max_orders_per_sec,
    );

    let mut backtester = Backtester::new(cfg.symbol.clone(), cfg.orderbook.depth_levels, strategy, risk);
    let result = backtester.run(events, snapshot);

    println!("=== Backtest Result ===");
    println!("Total trades : {}", result.total_trades);
    println!("Buy trades   : {}", result.buy_trades);
    println!("Sell trades  : {}", result.sell_trades);
    println!("Total volume : {:.4}", result.total_volume);
    println!("Realized PnL : {:.2} USDT", result.realized_pnl);
    println!("======================");

    Ok(())
}
