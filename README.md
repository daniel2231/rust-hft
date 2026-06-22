# crypto-trader

Low-latency cryptocurrency algorithmic trading system written in Rust.

## Architecture

Multi-threaded pipeline designed for minimal GC latency:

```
[Binance WebSocket] → ingestion → orderbook → strategy → risk → execution
```

- **Thread 1 (Tokio async):** WebSocket ingestion, REST snapshot fetch, order execution
- **Thread 2 (OS thread):** Orderbook maintenance, strategy evaluation, risk checks
- **Bridge:** `crossbeam-channel` between async and sync worlds

### 이벤트(Event)란?

로그에 나오는 "이벤트 1개"는 **바이낸스가 100ms(0.1초)마다 보내주는 호가 변동 묶음 1개**입니다.

바이낸스에서는 매 순간 수많은 주문이 들어오고, 취소되고, 체결됩니다. 이 변동을 건별로 모두 전송하면 데이터가 너무 많아지기 때문에, 바이낸스는 **0.1초 동안 쌓인 변동 내용을 하나로 묶어** 전송합니다.

예시:
```
"66,674.7 달러에 1.193 BTC 사겠다는 주문이 새로 들어왔다"
"66,800.0 달러에 0.5 BTC 팔겠다는 주문이 취소됐다"
```

이런 변동 내용의 묶음이 이벤트 1개입니다. BTCUSDT처럼 거래가 활발한 종목은 0.1초마다 거의 항상 변동이 있어 초당 약 10개의 이벤트가 꾸준히 들어옵니다.

## Milestones

| Milestone | Status | Description |
|-----------|--------|-------------|
| M1 | ✅ | Project setup, WebSocket connection, raw stream output |
| M2 | ✅ | Orderbook sync (Snapshot+Diff), unit tests |
| M3 | ✅ | Multi-thread pipeline, crossbeam channels, watchdog, graceful shutdown |
| M4 | ✅ | Risk checks + paper trading mode |
| M5 | ✅ | Backtesting harness |
| M6 | 🔲 | Strategy implementation, cloud VM deployment |

---

## Prerequisites

- Rust 1.82+
- (Optional) Binance API key for live mode

---

## 실행 방법

> **바이너리가 두 개**(`crypto-trader`, `backtest`)이므로 반드시 `--bin`을 명시해야 합니다.
>
> | 명령어 | 언제 사용 |
> |--------|-----------|
> | `cargo run --bin crypto-trader` | **실제 운영** — Binance WebSocket에 연결해 실시간 데이터 수신 |
> | `cargo run --bin backtest` | **전략 검증** — 과거 데이터를 파일로 불러와 PnL 시뮬레이션 |

### 1. Paper 트레이딩 모드로 실행 (기본)

실제 주문 없이 WebSocket으로 시장 데이터를 수신하고 전략 신호를 로그로만 기록합니다.

```bash
cp .env.example .env
RUST_LOG=info cargo run --bin crypto-trader
```

로그 레벨 조정:

```bash
RUST_LOG=debug cargo run --bin crypto-trader   # raw 메시지까지 출력
RUST_LOG=warn cargo run --bin crypto-trader    # 경고·오류만 출력
```

실행 시 출력 예시:

```
2026-06-15T09:00:00Z  INFO crypto_trader::ingestion: Connecting to wss://fstream.binance.com/ws/btcusdt@depth@100ms/...
2026-06-15T09:00:00Z  INFO crypto_trader::ingestion: WebSocket connected
2026-06-15T09:00:01Z  INFO crypto_trader::orderbook: Snapshot applied last_update_id=123456 symbol="BTCUSDT"
2026-06-15T09:00:01Z  INFO crypto_trader::main: [event=1000] best_bid=65000.10 best_ask=65001.20
```

Paper 트레이딩 주문 로그는 `logs/orders_paper.log`에 누적됩니다:

```
[PAPER] 2026-06-15T09:00:05Z | BUY | BTCUSDT | qty=0.0100 | price=65000.00 | signal=Buy | client_id=uuid-xxxx
```

### 2. 킬 스위치 (긴급 중단)

실행 중에 모든 주문을 즉시 차단하려면 프로젝트 루트에 `HALT` 파일을 생성합니다:

```bash
touch HALT        # 주문 차단 시작
rm HALT           # 주문 재개
```

### 3. 종료

`Ctrl+C`로 그레이스풀 셧다운됩니다.

---

## 테스트 방법

### 전체 테스트 실행

```bash
cargo test
```

예상 출력:

```
running 5 tests (orderbook_tests)
test tests::test_snapshot_apply ... ok
test tests::test_diff_apply ... ok
test tests::test_sequence_gap_detection ... ok
test tests::test_qty_zero_removes_level ... ok
test tests::test_buffering_state ... ok

running 5 tests (risk_tests)
test tests::test_kill_switch ... ok
test tests::test_max_order_qty ... ok
test tests::test_fat_finger_check ... ok
test tests::test_valid_order_passes ... ok
test tests::test_insufficient_balance ... ok

running 3 tests (backtest_tests)
test tests::test_synthetic_data_generation ... ok
test tests::test_backtester_noop_strategy ... ok
test tests::test_pnl_calculation ... ok

test result: ok. 13 passed; 0 failed
```

### 특정 테스트만 실행

```bash
cargo test orderbook        # 오더북 테스트만
cargo test risk             # 리스크 테스트만
cargo test backtest         # 백테스트 테스트만
cargo test test_pnl         # 이름에 "test_pnl"이 포함된 테스트만
```

### 테스트 출력 확인 (println! 포함)

```bash
cargo test -- --nocapture
```

---

## 백테스트

### 합성 데이터로 실행 (빠른 검증)

```bash
cargo run --bin backtest
```

출력 예시:

```
=== Backtest Result ===
Total trades : 0
Buy trades   : 0
Sell trades  : 0
Total volume : 0.0000
Realized PnL : 0.00 USDT
======================
```

> `NoOpStrategy`는 항상 Hold를 반환하므로 거래가 발생하지 않습니다. 전략 구현 후 교체하면 실제 PnL이 계산됩니다.

### 히스토리컬 데이터 파일로 실행

```bash
cargo run --bin backtest -- path/to/data.ndjson
```

파일 포맷: 한 줄에 하나의 Binance `depthUpdate` JSON 이벤트.

```json
{"e":"depthUpdate","E":1718445600000,"s":"BTCUSDT","U":100001,"u":100010,"pu":100000,"b":[["65000.00","1.5"]],"a":[["65001.00","2.0"]]}
{"e":"depthUpdate","E":1718445600100,"s":"BTCUSDT","U":100011,"u":100020,"pu":100010,"b":[["65000.00","0.0"]],"a":[["65002.00","1.0"]]}
```

---

## 설정

`config/default.toml` 수정:

```toml
mode = "paper"     # "paper" 또는 "live" (live는 M6 이후)
symbol = "BTCUSDT"

[risk]
max_order_qty = 0.01          # 최대 주문 수량 (BTC)
min_free_balance_usdt = 100.0 # 최소 잔고 (USDT)
price_band_pct = 1.0          # 팻 핑거 가드: mid price 대비 ±1% 초과 주문 차단
max_orders_per_sec = 5        # 초당 최대 주문 수

[orderbook]
depth_levels = 20             # 오더북 상위 N 레벨 유지

[watchdog]
timeout_secs = 30             # 30초간 메시지 없으면 자동 재연결

[exchange]
ws_url = "wss://fstream.binance.com/ws"
rest_url = "https://fapi.binance.com"
```

---

## Strategy 인터페이스

`src/strategy/` 디렉토리에서 `Strategy` trait 구현:

```rust
pub trait Strategy: Send {
    fn on_orderbook(&mut self, snapshot: &OrderbookSnapshot) -> Signal;
}
```

구현 가이드: `src/strategy/README.md` 참조.

---

## 보안

- API 키는 환경변수로만 관리 — `.env` 파일은 절대 커밋 금지
- Paper 모드는 거래소 API를 호출하지 않음

---

## 서버 배포 (24/7 운영)

리눅스 서버에서 죽지 않고 24시간 돌리려면 **systemd 서비스**로 등록하는 것이 가장 안정적입니다. 크래시 시 자동 재시작, 부팅 시 자동 시작, journald 로깅을 제공합니다.

### 자동 설치

레포 루트에서 (서버에서 root로 실행):

```bash
sudo deploy/install.sh
```

이 스크립트는 다음을 수행합니다:

1. `cargo build --release`로 바이너리 빌드
2. 전용 시스템 유저 `crypto` 생성
3. `/opt/crypto-trader`에 바이너리·`config/`·`.env` 설치 (`.env`는 `0600`)
4. systemd 유닛 등록 및 부팅 시 자동 시작 활성화

### 시작 / 상태 / 로그

```bash
sudo systemctl start crypto-trader      # 시작
sudo systemctl status crypto-trader     # 상태 확인
journalctl -u crypto-trader -f          # 실시간 로그
sudo systemctl restart crypto-trader    # 재시작
sudo systemctl stop crypto-trader       # 중단 (SIGINT → 그레이스풀 셧다운)
```

### 라이브 모드 자격증명

라이브 거래 시 `/opt/crypto-trader/.env`에 실제 API 키를 입력합니다:

```bash
sudo nano /opt/crypto-trader/.env       # BINANCE_API_KEY / BINANCE_API_SECRET
sudo systemctl restart crypto-trader
```

> 킬 스위치는 서버에서도 동작합니다: `sudo touch /opt/crypto-trader/HALT`

유닛 파일: [`deploy/crypto-trader.service`](deploy/crypto-trader.service) — 로그 레벨, 재시작 정책, 하드닝 옵션은 여기서 조정합니다.

---

## Phase Roadmap

| Phase | 환경 | RTT | 목적 |
|-------|------|-----|------|
| Phase 1 (현재) | Mac Mini 로컬 | ~40–80ms | 기능 개발 및 정확성 검증 |
| Phase 2 | AWS ap-northeast-1 (도쿄) | ~2–5ms | 전략 구현 후 실운영 |
