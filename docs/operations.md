---
title: Operations
---

# Operations

## Configuration

기본 설정 파일은 `config/default.toml`입니다.

```toml
mode = "paper"
symbol = "BTCUSDT"

[risk]
max_order_qty = 0.01
min_free_balance_usdt = 100.0
price_band_pct = 1.0
max_orders_per_sec = 5

[orderbook]
depth_levels = 20

[exchange]
ws_url = "wss://fstream.binance.com/ws"
rest_url = "https://fapi.binance.com"

[watchdog]
timeout_secs = 30

[dashboard]
port = 3000
enabled = true
```

## Paper Order Logs

Paper trading 주문은 `logs/orders_paper.log`에 누적됩니다.

```text
[PAPER] 2026-06-15T09:00:05Z | BUY | BTCUSDT | qty=0.0100 | price=65000.00 | signal=Buy | client_id=uuid-xxxx
```

## Kill Switch

프로젝트 루트에 `HALT` 파일이 있으면 주문이 차단됩니다.

```bash
touch HALT
```

주문을 다시 허용하려면 파일을 삭제합니다.

```bash
rm HALT
```

Dashboard가 활성화되어 있으면 `/halt`, `/resume` endpoint도 같은 동작을 수행합니다.

```text
http://localhost:3000/halt
http://localhost:3000/resume
```

## Watchdog

Watchdog는 설정된 시간 동안 market data가 들어오지 않으면 reconnect/resync 흐름을 유도합니다.

```toml
[watchdog]
timeout_secs = 30
```

## Security

- API key는 환경변수 또는 `.env`로 관리합니다.
- `.env`는 커밋하지 않습니다.
- Paper mode는 거래소 주문 API를 호출하지 않습니다.
- Live mode를 추가할 때는 주문 실행 경로에 별도 권한, rate limit, emergency stop을 둡니다.

## GitHub Pages Setup

1. Repository settings로 이동합니다.
2. Pages 메뉴에서 source를 `Deploy from a branch`로 선택합니다.
3. Branch는 `main`, folder는 `/docs`로 선택합니다.
4. Save 후 GitHub Pages build가 끝나면 문서 사이트가 공개됩니다.
