---
title: Configuration
---

# 설정

모든 런타임 설정은 `config/default.toml`에 있습니다. `crypto-trader`와 `backtest` 두 바이너리 모두 이 파일을 읽습니다.

## 전체 예시

```toml
mode = "paper"     # "paper" 또는 "live" (live는 M6 이후)
symbol = "BTCUSDT"
strategy = "momentum"   # "noop" | "pingpong" | "momentum"

[paper]
initial_capital_usdt = 730.0   # 가상 초기 자본 (USDT). 백만원 ≈ 730 USDT
fee_pct = 0.05                 # 체결당 수수료 % — momentum은 taker 체결이므로 0.05 (pingpong은 maker 0.02)

[risk]
max_order_qty = 0.01          # 최대 주문 수량 (기초자산 단위, BTC)
min_free_balance_usdt = 100.0 # 최소 가용 잔고 (USDT)
price_band_pct = 1.0          # 팻 핑거 가드: mid price 대비 ±1% 초과 주문 차단
max_orders_per_sec = 5        # 초당 최대 주문 수

[orderbook]
depth_levels = 20             # 오더북 상위 N 레벨 유지

[exchange]
ws_url = "wss://fstream.binance.com/ws"
rest_url = "https://fapi.binance.com"

[watchdog]
timeout_secs = 30             # N초간 메시지 없으면 자동 재연결

[dashboard]
port = 3000
enabled = true                # false면 대시보드 서버를 띄우지 않음
```

## 항목별 설명

### 최상위

| 키 | 기본값 | 설명 |
|----|--------|------|
| `mode` | `"paper"` | `paper`: 주문을 로그로만 기록. `live`: 실제 거래소 주문 (M6 이후 지원 예정) |
| `symbol` | `"BTCUSDT"` | 구독할 심볼. Binance USDT-M 선물 심볼 표기를 따릅니다 |
| `strategy` | `"noop"` | `crypto-trader`가 실행할 전략. `noop`(거래 없음), `pingpong`(테스트용 스프레드 캡처), `momentum`(모멘텀 스캘핑). 생략 시 `noop` |

### `[paper]` — 가상 계좌 (Paper 모드)

| 키 | 기본값 | 설명 |
|----|--------|------|
| `initial_capital_usdt` | `10000.0` | 가상 계좌의 시작 자본(USDT). 체결마다 현금이 차감/가산되고, 잔고를 초과하는 매수는 리스크 체크에서 거부됩니다 |
| `fee_pct` | `0.02` | 체결마다 체결 대금의 이 비율(%)만큼 수수료로 차감. Binance 선물 maker 0.02 / taker 0.05 |

대시보드의 수익률은 이 초기 자본 대비 현재 자산(현금 + 포지션 평가액)으로 계산되며 수수료가 반영됩니다. 프로세스를 재시작하면 계좌가 초기 자본으로 리셋됩니다. `pingpong`/`momentum` 전략의 주문 수량은 초기 자본의 90% 이내로 자동 조정됩니다.

### `[momentum]` — 모멘텀 스캘핑 전략 파라미터

`strategy = "momentum"`일 때 적용됩니다. `crypto-trader`와 `backtest` 바이너리가 이 값을 공유하므로, 백테스트에서 검증한 파라미터가 그대로 실전(paper)에 반영됩니다.

| 키 | 기본값 | 설명 |
|----|--------|------|
| `qty` | `0.01` | 진입 수량(BTC) — 자본 기준 상한(초기 자본의 90%)으로 추가 축소될 수 있음 |
| `lookback_ticks` | `30` | 모멘텀 관측 구간(오더북 틱 수, ~100ms/틱 → 기본 ≈3초) |
| `entry_mom_pct` | `0.05` | 진입 문턱: `lookback_ticks` 동안 mid 가격이 이 %만큼 올라야 진입 후보가 됨 |
| `imbalance_min` | `0.60` | 진입 확인: 오더북 상위 5레벨 매수 물량 비중(0~1)이 이 값 이상이어야 진입 |
| `tp_pct` | `0.20` | 익절: 진입가 대비 +tp_pct%. **왕복 수수료(taker 0.05%×2 = 0.10%)+스프레드보다 커야** 익절이 순익이 됩니다 |
| `stop_pct` | `0.10` | 손절: 진입가 대비 -stop_pct% |
| `max_hold_ticks` | `300` | 시간 손절 — 이 틱 수(기본 ≈30초)가 지나면 익절/손절 여부와 무관하게 강제 청산 |
| `cooldown_ticks` | `30` | 청산 직후 재진입을 금지하는 틱 수(기본 ≈3초) |

`entry_mom_pct`와 `imbalance_min`을 동시에 만족해야 진입하므로, 거래가 거의 없다면 두 값을 낮춰(예: `entry_mom_pct = 0.02`, `imbalance_min = 0.55`) 진입 빈도를 조정할 수 있습니다.

### `[pingpong]` — 핑퐁(스프레드 캡처) 전략 파라미터

`strategy = "pingpong"`일 때 적용됩니다.

| 키 | 기본값 | 설명 |
|----|--------|------|
| `qty` | `0.01` | 진입 수량(BTC) |
| `min_edge_pct` | `0.02` | 익절: 진입가(best bid) 대비 +min_edge_pct% |
| `stop_pct` | `0.5` | 손절: 진입가 대비 -stop_pct% |

### `[risk]` — 주문 안전장치

모든 Buy/Sell 신호는 실행 전에 아래 체크를 순서대로 통과해야 합니다. 하나라도 실패하면 주문이 거부되고 `warn` 로그가 남습니다.

| 키 | 기본값 | 체크 내용 |
|----|--------|-----------|
| `max_order_qty` | `0.01` | 주문 수량이 이 값을 넘으면 거부 |
| `min_free_balance_usdt` | `100.0` | 가용 잔고가 이 값 미만이면 거부 |
| `price_band_pct` | `1.0` | 주문 가격이 mid price에서 ±N% 이상 벗어나면 거부 (팻 핑거 방지) |
| `max_orders_per_sec` | `5` | 최근 1초간 주문 수가 이 값에 도달하면 거부. 80% 도달 시 경고 로그 |

이 외에 설정과 무관하게 항상 동작하는 체크: **킬 스위치**(`HALT` 파일), **중복 주문 ID 차단**.

### `[orderbook]`

| 키 | 기본값 | 설명 |
|----|--------|------|
| `depth_levels` | `20` | 메모리에 유지할 매수/매도 호가 레벨 수. 늘리면 메모리와 업데이트 비용 증가 |

### `[exchange]`

| 키 | 기본값 | 설명 |
|----|--------|------|
| `ws_url` | Binance 선물 WS | `depthUpdate` 스트림 수신 주소 |
| `rest_url` | Binance 선물 REST | 오더북 스냅샷 조회 주소 |

기본값은 Binance USDT-M 선물입니다. 테스트넷을 쓰려면 두 URL을 테스트넷 주소로 바꾸면 됩니다.

### `[watchdog]`

| 키 | 기본값 | 설명 |
|----|--------|------|
| `timeout_secs` | `30` | 이 시간 동안 WebSocket 메시지가 없으면 연결이 죽은 것으로 판단하고 재연결 |

### `[dashboard]`

| 키 | 기본값 | 설명 |
|----|--------|------|
| `enabled` | `true` | 웹 대시보드 서버 활성화 여부 |
| `port` | `3000` | HTTP/WebSocket 포트 |

## 환경변수

| 변수 | 용도 |
|------|------|
| `RUST_LOG` | 로그 레벨 (`debug` / `info` / `warn` / `error`) |
| `BINANCE_API_KEY` | 라이브 모드 API 키 (`.env` 파일 사용) |
| `BINANCE_API_SECRET` | 라이브 모드 API 시크릿 |

## 배포 환경의 설정 위치

systemd로 배포한 경우 설정 파일은 `/opt/crypto-trader/config/default.toml`입니다. 수정 후 재시작해야 반영됩니다:

```bash
sudo nano /opt/crypto-trader/config/default.toml
sudo systemctl restart crypto-trader
```
