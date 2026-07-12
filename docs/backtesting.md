# 백테스팅

`backtest` 바이너리는 과거(또는 합성) `depthUpdate` 이벤트를 실전과 **동일한 파이프라인**(orderbook → strategy → risk)에 흘려보내 전략의 PnL을 시뮬레이션합니다. 네트워크 연결이 필요 없습니다.

## 빠른 실행 — 합성 데이터

인자 없이 실행하면 결정적(deterministic) 합성 데이터 100개 이벤트를 생성해 돌립니다. 파이프라인 자체가 정상인지 빠르게 확인하는 용도입니다.

```bash
cargo run --bin backtest
```

출력:

```
=== Backtest Result ===
Total trades : 0
Buy trades   : 0
Sell trades  : 0
Total volume : 0.0000
Realized PnL : 0.00 USDT
======================
```

> 기본 `NoOpStrategy`는 항상 `Hold`를 반환하므로 거래가 0건입니다. 실제 전략으로 바꾸는 방법은 아래 [전략 교체](#전략-교체) 참조.

## 히스토리컬 데이터로 실행

```bash
cargo run --bin backtest -- path/to/data.ndjson
```

### 데이터 파일 포맷 (NDJSON)

한 줄에 하나의 Binance `depthUpdate` JSON 이벤트. 빈 줄은 무시되며, 파싱 불가능한 줄이 있으면 줄 번호와 함께 에러가 납니다.

```json
{"e":"depthUpdate","E":1718445600000,"s":"BTCUSDT","U":100001,"u":100010,"pu":100000,"b":[["65000.00","1.5"]],"a":[["65001.00","2.0"]]}
{"e":"depthUpdate","E":1718445600100,"s":"BTCUSDT","U":100011,"u":100020,"pu":100010,"b":[["65000.00","0.0"]],"a":[["65002.00","1.0"]]}
```

| 필드 | 의미 |
|------|------|
| `e` | 이벤트 타입 (`depthUpdate`) |
| `E` | 이벤트 시각 (ms epoch) |
| `s` | 심볼 |
| `U` / `u` / `pu` | first / last / previous-last update ID (시퀀스 검증에 사용) |
| `b` / `a` | 매수/매도 호가 변경 `[가격, 수량]` 목록. 수량 `"0.0"`은 해당 레벨 삭제 |

데이터 수집 방법: 라이브 모드로 `RUST_LOG=debug` 실행 시 출력되는 raw 메시지를 저장하거나, Binance 공식 히스토리컬 데이터를 이 포맷으로 변환합니다.

> **주의:** 파일 데이터로 실행할 때는 초기 스냅샷 없이 첫 이벤트부터 오더북을 구성하므로, update ID가 연속된 구간의 데이터를 사용해야 합니다. 시퀀스 갭이 있는 이벤트는 건너뜁니다.

## 백테스트 동작 방식

1. (합성 모드) 스냅샷을 오더북에 적용
2. 각 이벤트를 순서대로 오더북에 적용
3. 오더북이 갱신될 때마다 `strategy.on_orderbook(&snapshot)` 호출
4. `Buy`/`Sell` 신호는 **실전과 동일한 리스크 체크**(`config/default.toml`의 `[risk]`)를 거침 — 통과한 주문만 체결로 기록
5. 종료 후 FIFO 매칭으로 실현 PnL 계산

### PnL 계산 (FIFO)

각 Sell은 가장 오래된 Buy부터 순서대로 매칭됩니다:

```
PnL per match = (sell_price − buy_price) × min(buy_qty, sell_qty)
```

매칭되지 않은 잔여 포지션(unrealized)은 PnL에 포함되지 않습니다. 수수료와 슬리피지는 현재 모델링하지 않으므로, 결과는 낙관적 상한으로 해석해야 합니다.

## 전략 교체

`src/bin/backtest.rs`에서 `NoOpStrategy`를 자신의 전략으로 바꿉니다:

```rust
// let strategy = Box::new(NoOpStrategy);
let strategy = Box::new(MyStrategy::new());
```

전략 구현 방법은 [전략 개발](strategy-development.md)을 참조하세요. 같은 전략 구조체를 `src/main.rs`(실전)와 `src/bin/backtest.rs`(백테스트) 양쪽에서 사용할 수 있으므로, **백테스트에서 검증한 코드가 그대로 실전에 올라갑니다.**

## 백테스트 자체 테스트

```bash
cargo test backtest
```

합성 데이터 생성, NoOp 전략 무거래, FIFO PnL 계산을 검증하는 3개의 테스트가 포함되어 있습니다.

## 권장 워크플로우

1. `cargo run --bin backtest` — 합성 데이터로 파이프라인 동작 확인
2. 전략 구현 후 히스토리컬 데이터로 PnL 검증
3. 결과가 좋으면 Paper 모드로 실시간 검증 ([시작하기](getting-started.md))
4. 충분한 Paper 검증 후 라이브 전환 ([실전 운영](production.md))
