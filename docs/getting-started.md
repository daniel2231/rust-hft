# 시작하기

[설치](installation.md)를 마친 상태를 가정합니다.

## Paper 트레이딩 모드 실행 (기본)

Paper 모드는 Binance WebSocket으로 **실제 시장 데이터를 수신**하지만, 주문은 거래소로 보내지 않고 로그 파일에만 기록합니다. 전략과 파이프라인을 안전하게 검증하는 기본 모드입니다.

```bash
RUST_LOG=info cargo run --bin crypto-trader
```

정상 기동 시 로그:

```
INFO crypto_trader::main: crypto-trader starting mode=paper symbol=BTCUSDT
INFO crypto_trader::ingestion: Connecting to wss://fstream.binance.com/ws/btcusdt@depth@100ms/...
INFO crypto_trader::ingestion: WebSocket connected
INFO crypto_trader::orderbook: Snapshot applied last_update_id=123456 symbol="BTCUSDT"
INFO crypto_trader::main: [event=1000] best_bid=65000.10 best_ask=65001.20
```

### 로그 레벨 조정

```bash
RUST_LOG=debug cargo run --bin crypto-trader   # raw WebSocket 메시지까지 출력
RUST_LOG=info  cargo run --bin crypto-trader   # 기본 — 연결/스냅샷/1000이벤트 단위 통계
RUST_LOG=warn  cargo run --bin crypto-trader   # 경고·오류만
```

### "이벤트"란?

로그의 이벤트 1개는 **Binance가 100ms마다 보내는 호가 변동 묶음 1개**(`depthUpdate`)입니다. BTCUSDT처럼 활발한 종목은 초당 약 10개의 이벤트가 꾸준히 들어옵니다.

## 주문 로그 확인

전략이 Buy/Sell 신호를 내고 리스크 체크를 통과하면 `logs/orders_paper.log`에 기록됩니다:

```
[PAPER] 2026-06-15T09:00:05Z | BUY | BTCUSDT | qty=0.0100 | price=65000.00 | signal=Buy | client_id=uuid-xxxx
```

> 기본 탑재된 `NoOpStrategy`는 항상 `Hold`를 반환하므로 주문이 발생하지 않습니다. 주문을 보려면 [전략 개발](strategy-development.md)을 따라 전략을 구현하세요.

## 킬 스위치 (긴급 중단)

실행 중 모든 주문을 즉시 차단하려면 프로젝트 루트에 `HALT` 파일을 생성합니다:

```bash
touch HALT        # 주문 차단 시작 (데이터 수신은 계속됨)
rm HALT           # 주문 재개
```

백그라운드 태스크가 0.5초마다 파일 존재 여부를 폴링해 공유 플래그를 갱신하고, 리스크 체크는 이 플래그를 읽어 `KillSwitch` 오류로 주문을 거부합니다. 대시보드의 토글 버튼([대시보드](dashboard.md) 참조)으로도 같은 동작을 할 수 있습니다.

## 종료

`Ctrl+C`를 누르면 그레이스풀 셧다운됩니다.

## 연결 안정성

- **Watchdog:** `timeout_secs`(기본 30초) 동안 메시지가 없으면 자동 재연결합니다.
- **시퀀스 갭:** 오더북 업데이트 ID에 갭이 감지되면 자동으로 스냅샷을 다시 받아 재동기화합니다. 재동기화 중에는 대시보드 상태가 `Buffering`으로 표시됩니다.

## 테스트 실행

```bash
cargo test                    # 전체 (orderbook 5 + risk 5 + backtest 3 = 13개)
cargo test orderbook          # 오더북 테스트만
cargo test risk               # 리스크 테스트만
cargo test backtest           # 백테스트 테스트만
cargo test -- --nocapture     # println! 출력 포함
```

## 다음 단계

- 실행 중 상태를 눈으로 보려면 → [대시보드](dashboard.md)
- 파라미터를 바꾸려면 → [설정](configuration.md)
- 전략을 만들려면 → [전략 개발](strategy-development.md)
