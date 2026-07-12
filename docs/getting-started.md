---
title: Getting Started
---

# Getting Started

## Requirements

- Rust 1.82 이상
- Binance WebSocket 접근 가능한 네트워크
- Live mode를 사용할 경우 Binance API key

현재 기본 모드는 `paper`이며 실제 거래소 주문 API를 호출하지 않습니다.

## Install

```bash
git clone <repository-url>
cd rust-hft
cp .env.example .env
```

## Run Paper Trading

```bash
RUST_LOG=info cargo run --bin crypto-trader
```

로그 레벨은 필요에 따라 조정할 수 있습니다.

```bash
RUST_LOG=debug cargo run --bin crypto-trader
RUST_LOG=warn cargo run --bin crypto-trader
```

## Dashboard

`config/default.toml`에서 dashboard가 활성화되어 있으면 기본 포트 `3000`에서 상태 페이지가 열립니다.

```toml
[dashboard]
port = 3000
enabled = true
```

실행 후 브라우저에서 다음 주소를 엽니다.

```text
http://localhost:3000
```

## Tests

전체 테스트:

```bash
cargo test
```

특정 영역만 실행:

```bash
cargo test orderbook
cargo test risk
cargo test backtest
```

테스트 출력까지 확인:

```bash
cargo test -- --nocapture
```

## Binaries

| Command | Use case |
| --- | --- |
| `cargo run --bin crypto-trader` | Binance WebSocket에 연결해 paper trading pipeline 실행 |
| `cargo run --bin backtest` | 합성 데이터 또는 파일 기반 백테스트 실행 |
