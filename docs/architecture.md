---
title: Architecture
---

# Architecture

## Runtime Model

`crypto-trader`는 async I/O와 latency-sensitive 처리를 분리합니다.

| Area | Runtime | Responsibility |
| --- | --- | --- |
| Ingestion | Tokio async task | WebSocket 수신, REST snapshot 조회, reconnect 처리 |
| Orderbook + Strategy | Dedicated OS thread | 오더북 업데이트, strategy signal 생성, risk check |
| Execution | Tokio task | validated order 처리, paper execution 로그 기록 |
| Dashboard | Axum server | 상태 조회, WebSocket push, halt/resume endpoint |

Async와 sync 영역은 `crossbeam-channel`로 연결됩니다. 시장 이벤트 채널은 `bounded(1024)`, 주문 채널은 `bounded(64)`입니다.

## Data Flow

```text
Binance depth stream
        |
        v
ingestion::run_ingestion
        |
        v
MarketEvent channel
        |
        v
orderbook::Orderbook
        |
        v
strategy::Strategy::on_orderbook
        |
        v
risk::RiskChecker
        |
        v
execution::PaperExecutor
```

## Market Events

로그에 나오는 이벤트 1개는 Binance가 100ms 단위로 보내는 depth update 묶음입니다. BTCUSDT처럼 거래가 활발한 심볼은 초당 약 10개의 depth update가 꾸준히 들어올 수 있습니다.

`MarketEvent`는 주요 이벤트를 하나의 파이프라인 타입으로 모읍니다.

| Event | Meaning |
| --- | --- |
| `DepthUpdate` | WebSocket diff depth update |
| `DepthSnapshot` | REST snapshot |
| `Trade` | Trade event |
| `Reconnect` | reconnect 또는 resync 필요 신호 |

## Orderbook Sync

오더북은 Binance snapshot + diff update 모델을 따릅니다.

1. REST snapshot을 받아 `last_update_id`를 저장합니다.
2. WebSocket diff update를 sequence 기준으로 적용합니다.
3. gap이 감지되면 오더북을 reset하고 resync를 요청합니다.
4. live 상태가 되면 strategy에 snapshot을 전달합니다.

## Risk Gates

Strategy가 `Buy` 또는 `Sell` signal을 반환하면 `RiskChecker`가 다음 조건을 검사합니다.

| Check | Purpose |
| --- | --- |
| Max order quantity | 비정상적으로 큰 주문 차단 |
| Minimum free balance | paper/live 실행 전 잔고 기준 검증 |
| Price band | mid price 대비 과도하게 벗어난 가격 차단 |
| Orders per second | 단기 폭주 방지 |
| Kill switch | `HALT` 파일 존재 시 주문 차단 |

## Dashboard State

Dashboard는 shared state를 통해 event count, connection status, live orderbook status, 최근 가격 히스토리, paper execution 결과를 표시합니다. WebSocket endpoint는 1초마다 상태 JSON을 push합니다.
