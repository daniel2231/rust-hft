# 설정

모든 런타임 설정은 `config/default.toml`에 있습니다. `crypto-trader`와 `backtest` 두 바이너리 모두 이 파일을 읽습니다.

## 전체 예시

```toml
mode = "paper"     # "paper" 또는 "live" (live는 M6 이후)
symbol = "BTCUSDT"
strategy = "pingpong"   # "noop" | "pingpong"

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
| `strategy` | `"noop"` | `crypto-trader`가 실행할 전략. `noop`(거래 없음) 또는 `pingpong`(테스트용 스프레드 캡처). 생략 시 `noop` |

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
