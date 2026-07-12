---
title: Dashboard
---

# 실시간 웹 대시보드

`crypto-trader` 실행 시 `config/default.toml`의 `[dashboard]`가 활성화되어 있으면(기본값) axum 기반 HTTP/WebSocket 서버가 함께 뜹니다.

```bash
cargo run --bin crypto-trader
# INFO crypto_trader::dashboard::server: Dashboard running at http://localhost:3000
```

브라우저에서 `http://localhost:3000` 접속.

## 표시 정보

- **실시간 가격 차트** — 최근 5분, 300개 포인트
- **오더북** — 매수/매도 상위 10레벨
- **상태 지표** — 초당 이벤트 수, 연결 상태, 동기화 상태(`Buffering`/`Live`)
- **페이퍼 트레이딩** — 최근 체결 내역, 매수/매도 카운트
- **킬 스위치 토글 버튼** — 주문 차단/재개

상태는 WebSocket으로 1초마다 push됩니다.

## 엔드포인트

| 경로 | 용도 |
|------|------|
| `GET /` | 대시보드 HTML (`static/dashboard.html`) |
| `GET /ws` | 상태 push WebSocket (1초 주기) |
| `GET /halt` | 킬 스위치 활성화 (= `touch HALT`) |
| `GET /resume` | 킬 스위치 해제 (= `rm HALT`) |

킬 스위치는 파일 기반이므로 대시보드 버튼과 셸의 `touch HALT` / `rm HALT`는 완전히 동일하게 동작합니다.

## 설정

```toml
[dashboard]
port = 3000
enabled = true    # false로 두면 대시보드 서버를 띄우지 않음
```

## 설계 노트

대시보드는 핫 패스 밖에 있습니다. 오더북 스레드가 최신 상태를 atomics + `RwLock`으로 공유하고, 대시보드는 1초에 한 번 읽어 push할 뿐이므로 트레이딩 지연에 영향을 주지 않습니다.

> ⚠️ **서버 배포 시 주의:** 대시보드에는 인증이 없습니다. 킬 스위치 조작이 가능하므로 공개 인터넷에 포트를 노출하지 말고, SSH 터널(`ssh -L 3000:localhost:3000 server`)이나 방화벽으로 접근을 제한하세요.
