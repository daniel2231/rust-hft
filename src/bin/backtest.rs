use anyhow::Result;
use crypto_trader::backtest::data_loader;
use crypto_trader::backtest::{Backtester, TradeSide};
use crypto_trader::config;
use crypto_trader::risk::RiskChecker;
use crypto_trader::strategy::{MomentumScalpStrategy, NoOpStrategy, PingPongStrategy, Strategy};

fn main() -> Result<()> {
    let cfg = config::load_config("config/default.toml")?;

    // Usage: backtest [data.ndjson] [--strategy noop|pingpong|momentum] [--events N]
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut data_path: Option<String> = None;
    let mut strategy_name = "noop".to_string();
    let mut num_events: usize = 100;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--strategy" => {
                strategy_name = args
                    .get(i + 1)
                    .ok_or_else(|| anyhow::anyhow!("--strategy requires a value"))?
                    .clone();
                i += 2;
            }
            "--events" => {
                num_events = args
                    .get(i + 1)
                    .ok_or_else(|| anyhow::anyhow!("--events requires a value"))?
                    .parse()?;
                i += 2;
            }
            other => {
                data_path = Some(other.to_string());
                i += 1;
            }
        }
    }

    let (snapshot, events) = match &data_path {
        Some(path) => {
            let events = data_loader::load_depth_updates(path)?;
            (None, events)
        }
        None => {
            let (snap, evts) = data_loader::generate_synthetic_data(&cfg.symbol, num_events);
            (Some(snap), evts)
        }
    };

    let strategy: Box<dyn Strategy> = match strategy_name.as_str() {
        "noop" => Box::new(NoOpStrategy),
        "pingpong" => Box::new(PingPongStrategy::default()),
        "momentum" => Box::new(MomentumScalpStrategy::default()),
        other => anyhow::bail!("Unknown strategy '{}' (available: noop, pingpong, momentum)", other),
    };

    let risk = RiskChecker::new(
        cfg.risk.max_order_qty,
        cfg.risk.min_free_balance_usdt,
        cfg.risk.price_band_pct,
        cfg.risk.max_orders_per_sec,
    );

    let mut backtester = Backtester::new(cfg.symbol.clone(), cfg.orderbook.depth_levels, strategy, risk);
    let result = backtester.run(events, snapshot);

    let buy_notional: f64 = result
        .trades
        .iter()
        .filter(|t| matches!(t.side, TradeSide::Buy))
        .map(|t| t.price * t.qty)
        .sum();
    let return_pct = if buy_notional > 0.0 {
        result.realized_pnl / buy_notional * 100.0
    } else {
        0.0
    };

    println!("=== Backtest Result ===");
    println!("Strategy     : {}", strategy_name);
    println!("Total trades : {}", result.total_trades);
    println!("Buy trades   : {}", result.buy_trades);
    println!("Sell trades  : {}", result.sell_trades);
    println!("Total volume : {:.4}", result.total_volume);
    println!("Buy notional : {:.2} USDT", buy_notional);
    println!("Realized PnL : {:.2} USDT", result.realized_pnl);
    println!("Return       : {:.4}% (PnL / buy notional)", return_pct);
    println!("======================");

    Ok(())
}
