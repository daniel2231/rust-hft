---
title: Backtesting
---

# Backtesting

Backtest binary는 strategy 구현 전후에 PnL 계산과 pipeline behavior를 빠르게 확인하기 위한 도구입니다.

## Run With Synthetic Data

```bash
cargo run --bin backtest
```

기본 `NoOpStrategy`는 항상 `Hold`를 반환하므로 거래가 발생하지 않습니다. Strategy를 교체하면 buy/sell trade, volume, realized PnL이 계산됩니다.

예상 출력 형태:

```text
=== Backtest Result ===
Total trades : 0
Buy trades   : 0
Sell trades  : 0
Total volume : 0.0000
Realized PnL : 0.00 USDT
======================
```

## Run With Historical Data

```bash
cargo run --bin backtest -- path/to/data.ndjson
```

파일은 newline-delimited JSON이어야 하며, 한 줄에 하나의 Binance `depthUpdate` 이벤트를 둡니다.

```json
{"e":"depthUpdate","E":1718445600000,"s":"BTCUSDT","U":100001,"u":100010,"pu":100000,"b":[["65000.00","1.5"]],"a":[["65001.00","2.0"]]}
{"e":"depthUpdate","E":1718445600100,"s":"BTCUSDT","U":100011,"u":100020,"pu":100010,"b":[["65000.00","0.0"]],"a":[["65002.00","1.0"]]}
```

## What To Validate

| Area | Check |
| --- | --- |
| Strategy behavior | 의도한 market condition에서 `Buy`, `Sell`, `Hold`가 나오는지 확인 |
| PnL | realized PnL이 strategy 변경에 따라 합리적으로 변하는지 확인 |
| Order count | signal 폭주나 예상보다 많은 trade가 없는지 확인 |
| Data assumptions | historical file의 symbol, sequence, bid/ask shape가 parser와 맞는지 확인 |
