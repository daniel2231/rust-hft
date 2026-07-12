# 설치

## 사전 요구사항

- **Rust 1.82+** — [rustup](https://rustup.rs/)으로 설치
- **인터넷 연결** — Binance WebSocket(`fstream.binance.com`) 접속용
- (선택) **Binance API 키** — 라이브 모드에서만 필요. Paper 모드는 키 없이 동작합니다.

지원 플랫폼: Linux, macOS. (Phase 1은 Mac Mini 로컬 개발, Phase 2는 AWS 도쿄 리전 배포를 상정합니다.)

## 소스 빌드

```bash
git clone https://github.com/daniel2231/rust-hft.git
cd rust-hft
cargo build --release
```

빌드 결과물:

- `target/release/crypto-trader` — 실시간 트레이딩 바이너리
- `target/release/backtest` — 백테스트 바이너리

개발 중에는 `cargo run --bin <이름>`으로 빌드+실행을 한 번에 할 수 있습니다. 바이너리가 두 개이므로 **반드시 `--bin`을 명시**해야 합니다.

## 환경변수 설정

```bash
cp .env.example .env
```

`.env` 내용:

```bash
BINANCE_API_KEY=your_api_key_here
BINANCE_API_SECRET=your_api_secret_here
```

- **Paper 모드(기본)**: 거래소 API를 호출하지 않으므로 placeholder 값 그대로 둬도 됩니다.
- **Live 모드**: 실제 Binance API 키/시크릿을 입력해야 합니다. [실전 운영](production.md) 참조.

> ⚠️ `.env` 파일은 절대 커밋하지 마세요. API 키는 환경변수로만 관리합니다.

## 설치 확인

```bash
# 전체 테스트 (13개 통과해야 정상)
cargo test

# 합성 데이터 백테스트 (네트워크 불필요, 몇 초 내 완료)
cargo run --bin backtest
```

`=== Backtest Result ===` 블록이 출력되면 설치가 정상입니다.

## 서버 설치 (systemd)

리눅스 서버에 24/7 서비스로 설치하려면 [실전 운영](production.md)의 자동 설치 스크립트(`sudo deploy/install.sh`)를 사용하세요.

## 다음 단계

[시작하기](getting-started.md)에서 Paper 모드로 실시간 데이터를 수신해 봅니다.
