---
title: Backtesting
---

# 백테스팅

`backtest` 바이너리는 과거 또는 합성 `depthUpdate` 이벤트를 실전과 **동일한 파이프라인**인 orderbook, strategy, risk에 흘려보내 전략의 PnL을 시뮬레이션합니다. 네트워크 연결은 필요 없습니다.

## 사용법

```text
cargo run --bin backtest -- [data.ndjson] [--strategy noop|pingpong] [--events N]
```

| 옵션 | 기본값 | 설명 |
|------|--------|------|
| `data.ndjson` | (없음 → 합성 데이터) | 히스토리컬 데이터 파일 경로 |
| `--strategy` | `noop` | 전략 선택: `noop` / `pingpong`(스프레드 캡처) / `momentum`(모멘텀 스캘핑) |
| `--events` | `100` | 합성 데이터 모드에서 생성할 이벤트 수 (100ms 간격 → 10000개 ≈ 약 17분 분량) |

## 빠른 실행 — 합성 데이터

인자 없이 실행하면 시드 고정된 랜덤워크 합성 데이터 100개 이벤트를 생성해 돌립니다(실행마다 동일한 결과). 파이프라인 자체가 정상인지 빠르게 확인하는 용도입니다.

```bash
cargo run --bin backtest
```

> 기본 `NoOpStrategy`는 항상 `Hold`를 반환하므로 거래가 0건입니다.

## 내장 테스트 전략 — Ping-Pong

`PingPongStrategy`(`src/strategy/ping_pong.rs`)는 수익률 리포팅을 시험해 볼 수 있는 내장 전략입니다: 포지션이 없으면 best bid에 매수하고, best ask가 진입가 대비 +0.02%를 넘으면 매도(익절), best bid가 −0.5% 아래로 내려가면 매도(손절)합니다.

```bash
cargo run --bin backtest -- --strategy pingpong --events 10000
```

출력 예시:

```text
=== Backtest Result ===
Strategy     : pingpong
Total trades : 2171
Buy trades   : 1086
Sell trades  : 1085
Total volume : 21.7100
Buy notional : 703721.16 USDT
Realized PnL : 215.36 USDT
Return       : 0.0306% (PnL / buy notional)
======================
```

- **Buy notional**: 총 매수 대금 (체결가 × 수량의 합)
- **Return**: 실현 PnL(gross)을 총 매수 대금으로 나눈 수익률
- **Fees / Net PnL**: `[paper] fee_pct` 기준 수수료와 이를 차감한 순손익
- **Round trips**: 라운드트립(진입→청산) 횟수와 승/패 분해 — 승패는 **수수료 반영 순손익** 기준
- **Win rate / Avg hold / Profit factor**: 승률, 평균 보유 시간(이벤트 타임스탬프 기준), 총이익/총손실 비율

> 승률이 예상보다 크게 낮다면 익절 폭이 왕복 비용(수수료 2회 + 스프레드)보다 작지 않은지부터 확인하세요. 예: `tp_pct = 0.08`에 taker 수수료 0.05%면 왕복 수수료만 0.10%라 익절해도 순손실입니다.

### ⚠️ 결과 해석 시 주의 (모델 한계)

이 백테스터는 단순화된 체결 모델을 사용합니다. 결과는 **낙관적 상한**으로 해석하세요:

1. **체결 가정** — 리스크 체크를 통과한 주문은 전량 신호 가격에 체결된 것으로 간주합니다. 패시브 주문(best bid 매수)의 실제 체결 확률, 큐 순서, 부분 체결은 모델링하지 않습니다.
2. **수수료·슬리피지 없음** — 예컨대 Binance 선물 메이커 수수료 0.02%만 반영해도 위 예시의 엣지(0.03%) 대부분이 사라집니다.
3. **주문 레이트 리밋이 벽시계 기준** — 리스크 체크의 `max_orders_per_sec`는 실제 경과 시간을 사용하므로, 수 밀리초 만에 끝나는 백테스트에서는 주문이 초당 5건(기본값)에서 잘립니다. 거래 빈도가 높은 전략을 백테스트할 때는 `config/default.toml`의 `max_orders_per_sec`를 임시로 크게 올리고 돌리세요 (라이브 전환 전에 원복 필수).

## 히스토리컬 데이터로 실행

```bash
cargo run --bin backtest -- path/to/data.ndjson
```

## 데이터 파일 포맷

한 줄에 하나의 Binance `depthUpdate` JSON 이벤트를 둡니다. 빈 줄은 무시되며, 파싱 불가능한 줄이 있으면 줄 번호와 함께 에러가 납니다.

```json
{"e":"depthUpdate","E":1718445600000,"s":"BTCUSDT","U":100001,"u":100010,"pu":100000,"b":[["65000.00","1.5"]],"a":[["65001.00","2.0"]]}
{"e":"depthUpdate","E":1718445600100,"s":"BTCUSDT","U":100011,"u":100020,"pu":100010,"b":[["65000.00","0.0"]],"a":[["65002.00","1.0"]]}
```

| 필드 | 의미 |
| --- | --- |
| `e` | 이벤트 타입, `depthUpdate` |
| `E` | 이벤트 시각, ms epoch |
| `s` | 심볼 |
| `U` / `u` / `pu` | first / last / previous-last update ID |
| `b` / `a` | 매수/매도 호가 변경 `[가격, 수량]` 목록. 수량 `0.0`은 해당 레벨 삭제 |

데이터 수집 방법은 라이브 모드로 `RUST_LOG=debug` 실행 시 출력되는 raw 메시지를 저장하거나, Binance 공식 히스토리컬 데이터를 이 포맷으로 변환하는 방식입니다.

> 파일 데이터로 실행할 때는 초기 스냅샷 없이 첫 이벤트부터 오더북을 구성하므로, update ID가 연속된 구간의 데이터를 사용해야 합니다. 시퀀스 갭이 있는 이벤트는 건너뜁니다.

## 백테스트 동작 방식

1. 합성 모드에서는 스냅샷을 오더북에 적용합니다.
2. 각 이벤트를 순서대로 오더북에 적용합니다.
3. 오더북이 갱신될 때마다 `strategy.on_orderbook(&snapshot)`을 호출합니다.
4. `Buy` 또는 `Sell` 신호는 실전과 동일한 리스크 체크를 거칩니다.
5. 종료 후 FIFO 매칭으로 실현 PnL을 계산합니다.

## PnL 계산

각 Sell은 가장 오래된 Buy부터 순서대로 매칭됩니다.

```text
PnL per match = (sell_price - buy_price) * min(buy_qty, sell_qty)
```

매칭되지 않은 잔여 포지션은 realized PnL에 포함되지 않습니다.

## 전략 교체

자신의 전략을 백테스트하려면 `src/bin/backtest.rs`의 전략 선택 `match`에 한 줄 추가합니다.

```rust
let strategy: Box<dyn Strategy> = match strategy_name.as_str() {
    "noop" => Box::new(NoOpStrategy),
    "pingpong" => Box::new(PingPongStrategy::default()),
    "mystrategy" => Box::new(MyStrategy::new()),   // 추가
    ...
};
```

전략 구현 방법은 [전략 개발](../strategy-development/)을 참조하세요. 같은 전략 구조체를 `src/main.rs`와 `src/bin/backtest.rs` 양쪽에서 사용할 수 있으므로, 백테스트에서 검증한 코드가 그대로 실전에 올라갑니다.

## 백테스트 자체 테스트

```bash
cargo test backtest
```

합성 데이터 생성, NoOp 전략 무거래, FIFO PnL 계산을 검증하는 테스트가 포함되어 있습니다.

## 권장 워크플로우

1. `cargo run --bin backtest`로 합성 데이터 파이프라인 동작을 확인합니다.
2. 전략 구현 후 히스토리컬 데이터로 PnL을 검증합니다.
3. 결과가 좋으면 Paper 모드로 실시간 검증합니다.
4. 충분한 Paper 검증 후 라이브 전환을 검토합니다.
