---
title: Strategy Development
---

# 전략 개발

전략은 `src/strategy/` 디렉토리의 `Strategy` trait을 구현해 작성합니다.

## Strategy trait

```rust
pub trait Strategy: Send {
    fn on_orderbook(&mut self, snapshot: &OrderbookSnapshot) -> Signal;
}
```

- `on_orderbook`은 오더북이 갱신될 때마다(이벤트당 1회) 호출됩니다.
- 반환값 `Signal`:
  - `Signal::Hold` — 아무것도 하지 않음 (리스크 체크도 건너뜀)
  - `Signal::Buy { price, qty }` / `Signal::Sell { price, qty }` — 리스크 체크 → 실행 파이프라인으로 전달

## 구현 절차

1. `src/strategy/my_strategy.rs` 파일 생성:

```rust
use crate::types::{OrderbookSnapshot, Signal};
use super::Strategy;

pub struct MyStrategy {
    // 전략 상태 (이동평균 버퍼 등)
}

impl Strategy for MyStrategy {
    fn on_orderbook(&mut self, snapshot: &OrderbookSnapshot) -> Signal {
        // snapshot.bids / snapshot.asks: 상위 N레벨 (가격, 수량) 목록
        Signal::Hold
    }
}
```

2. `src/strategy/mod.rs`에 모듈 등록 및 export
3. `src/main.rs`(실전)와 `src/bin/backtest.rs`(백테스트)에서 `NoOpStrategy`를 교체:

```rust
let mut strat: Box<dyn strategy::Strategy> = Box::new(strategy::MyStrategy::new());
```

## 실행 환경과 성능 계약

`on_orderbook`은 **전용 OS 스레드**(async 아님)에서, 핫 패스 위에서 호출됩니다. 반드시 지켜야 할 규칙:

- **블로킹 금지.** 마이크로초 단위로 반환해야 합니다. 디스크/네트워크 IO, sleep, lock 대기를 넣지 마세요.
- **할당 최소화.** 이벤트마다 호출되므로 힙 할당(`Vec::new`, `String`, `.clone()`)을 루프 안에서 반복하지 마세요. 상태 버퍼는 생성자에서 `with_capacity`로 미리 잡고 재사용합니다.
- **로깅은 `tracing`만.** async·버퍼링되어 안전합니다. `println!`이나 동기 파일 쓰기는 금지.
- 현재 이벤트 빈도는 심볼당 초당 ~10회지만, 멀티 심볼 라이브 환경에서는 훨씬 높아질 수 있다는 전제로 작성하세요.

자세한 핫 패스 규칙은 리포지토리 루트의 `CLAUDE.md`를 참조하세요.

## 신호 이후의 흐름

전략이 `Buy`/`Sell`을 반환하면:

1. **리스크 체크** (`src/risk.rs`) — 킬 스위치, 최대 수량, 잔고, 팻 핑거 가격 밴드, 초당 주문 수, 중복 ID를 검사. 하나라도 실패하면 주문이 조용히 거부되고 `warn` 로그가 남습니다. 한도는 [설정](configuration.md)의 `[risk]`에서 조정합니다.
2. **실행** (`src/execution.rs`) — Paper 모드에서는 `logs/orders_paper.log`에 기록. Live 모드에서는 거래소 주문(M6 이후).

## 검증 순서

1. **백테스트** — [백테스팅](backtesting.md)을 따라 합성/히스토리컬 데이터로 PnL 확인
2. **Paper 모드** — 실시간 데이터로 신호 빈도, 리스크 거부율, 주문 로그 확인
3. **Live** — [실전 운영](production.md) 참조

같은 전략 구조체가 세 단계 모두에서 동일하게 사용되므로 코드 이식 작업이 없습니다.
