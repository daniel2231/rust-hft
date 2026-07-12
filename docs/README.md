# crypto-trader 문서

Rust로 작성된 저지연 암호화폐 알고리즘 트레이딩 시스템의 사용 문서입니다.

## 목차

| 문서 | 내용 |
|------|------|
| [설치](installation.md) | 사전 요구사항, 빌드, 환경변수 설정 |
| [시작하기](getting-started.md) | Paper 트레이딩 실행, 로그 확인, 킬 스위치, 테스트 |
| [설정](configuration.md) | `config/default.toml` 전체 옵션 레퍼런스 |
| [백테스팅](backtesting.md) | 합성 데이터 / 히스토리컬 데이터로 전략 검증 |
| [전략 개발](strategy-development.md) | `Strategy` trait 구현 가이드와 성능 규칙 |
| [대시보드](dashboard.md) | 실시간 웹 대시보드 사용법 |
| [실전 운영](production.md) | systemd 배포, 라이브 모드, 24/7 운영 체크리스트 |

## 처음이라면

1. [설치](installation.md)를 따라 빌드합니다.
2. [시작하기](getting-started.md)의 Paper 모드로 실시간 데이터 수신을 확인합니다.
3. [전략 개발](strategy-development.md)을 참고해 전략을 구현합니다.
4. [백테스팅](backtesting.md)으로 과거 데이터에서 PnL을 검증합니다.
5. 검증이 끝나면 [실전 운영](production.md)을 따라 서버에 배포합니다.

## 시스템 개요

```
[Binance WebSocket] → ingestion → orderbook → strategy → risk → execution
```

- **Thread 1 (Tokio async):** WebSocket 수신, REST 스냅샷, 주문 실행, 대시보드 서버
- **Thread 2 (OS thread):** 오더북 유지, 전략 평가, 리스크 체크
- **브리지:** `crossbeam-channel`로 async ↔ sync 연결

바이너리는 두 개입니다:

| 바이너리 | 용도 |
|----------|------|
| `crypto-trader` | 실시간 운영 — Binance WebSocket 연결, Paper/Live 트레이딩 |
| `backtest` | 전략 검증 — 과거 데이터로 PnL 시뮬레이션 |
