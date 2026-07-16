use anyhow::Result;
use crypto_trader::backtest::data_loader;
use crypto_trader::backtest::{Backtester, TradeSide};
use crypto_trader::config;
use crypto_trader::pnl::PaperAccount;
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
        "pingpong" => {
            let p = &cfg.pingpong;
            Box::new(PingPongStrategy::new(p.qty, p.min_edge_pct, p.stop_pct))
        }
        "momentum" => {
            let m = &cfg.momentum;
            Box::new(MomentumScalpStrategy::new(
                m.qty,
                m.lookback_ticks,
                m.entry_mom_pct,
                m.imbalance_min,
                m.tp_pct,
                m.stop_pct,
                m.max_hold_ticks,
                m.cooldown_ticks,
            ))
        }
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

    // Replay fills through a paper account (same capital/fee settings as
    // live paper mode) for fee-adjusted stats: win rate, hold time, net PnL.
    let mut account = PaperAccount::new(cfg.paper.initial_capital_usdt, cfg.paper.fee_pct);
    for t in &result.trades {
        match t.side {
            TradeSide::Buy => account.on_buy(t.price, t.qty, t.timestamp_ms),
            TradeSide::Sell => account.on_sell(t.price, t.qty, t.timestamp_ms),
        }
    }
    let stats = *account.stats();
    let net_pnl = result.realized_pnl - account.total_fees();

    println!("=== Backtest Result ===");
    println!("Strategy     : {}", strategy_name);
    println!("Total trades : {}", result.total_trades);
    println!("Buy trades   : {}", result.buy_trades);
    println!("Sell trades  : {}", result.sell_trades);
    println!("Total volume : {:.4}", result.total_volume);
    println!("Buy notional : {:.2} USDT", buy_notional);
    println!("Realized PnL : {:.2} USDT (gross)", result.realized_pnl);
    println!("Fees         : {:.2} USDT ({}%/fill)", account.total_fees(), cfg.paper.fee_pct);
    println!("Net PnL      : {:.2} USDT", net_pnl);
    println!("Return       : {:.4}% (gross PnL / buy notional)", return_pct);
    println!("--- Round trips (net of fees) ---");
    println!("Round trips  : {} (win {} / loss {})", stats.round_trips, stats.wins, stats.losses);
    println!("Win rate     : {:.1}%", stats.win_rate_pct());
    println!("Avg hold     : {:.1}s", stats.avg_hold_secs());
    if stats.gross_loss > 0.0 {
        println!("Profit factor: {:.2}", stats.gross_profit / stats.gross_loss);
    }
    println!("======================");

    Ok(())
}
