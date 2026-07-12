---
title: Strategy
---

# Strategy

Strategy는 `src/strategy/` 안에서 `Strategy` trait을 구현하는 방식으로 추가합니다.

```rust
pub trait Strategy: Send {
    fn on_orderbook(&mut self, snapshot: &OrderbookSnapshot) -> Signal;
}
```

## Implement A Strategy

새 파일을 만들고 trait을 구현합니다.

```rust
use crate::types::{OrderbookSnapshot, Signal};
use super::Strategy;

pub struct MyStrategy {
    // strategy state
}

impl Strategy for MyStrategy {
    fn on_orderbook(&mut self, snapshot: &OrderbookSnapshot) -> Signal {
        let _best_bid = snapshot.bids.first();
        let _best_ask = snapshot.asks.first();

        Signal::Hold
    }
}
```

그 다음 `src/main.rs`에서 `NoOpStrategy` 대신 새 strategy를 연결합니다.

```rust
let mut strat: Box<dyn strategy::Strategy> = Box::new(strategy::NoOpStrategy);
```

## Contract

`on_orderbook`는 오더북 업데이트마다 dedicated OS thread에서 호출됩니다. 이 함수는 매우 자주 실행되므로 다음 계약을 지켜야 합니다.

| Rule | Reason |
| --- | --- |
| Blocking I/O 금지 | market event 처리 지연 방지 |
| Allocation 최소화 | jitter와 latency spike 감소 |
| 빠른 반환 | microsecond 단위 처리를 목표로 유지 |
| 외부 상태 접근 최소화 | lock contention과 race condition 방지 |
| `Hold`를 기본값으로 사용 | 조건이 명확하지 않을 때 주문을 내지 않음 |

## Signals

| Signal | Meaning |
| --- | --- |
| `Hold` | 아무 주문도 생성하지 않음 |
| `Buy` | risk check 통과 시 buy order 후보 생성 |
| `Sell` | risk check 통과 시 sell order 후보 생성 |

`Buy` 또는 `Sell` signal은 곧바로 실행되지 않습니다. 먼저 `RiskChecker`가 수량, 가격 밴드, 잔고, rate limit, kill switch를 확인합니다.
