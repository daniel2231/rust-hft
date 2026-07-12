---
title: Overview
---

# crypto-trader

<p class="lead">Rust로 작성한 저지연 암호화폐 알고리즘 트레이딩 시스템입니다. Binance WebSocket 시장 데이터를 수신하고, 오더북을 동기화한 뒤 전략, 리스크 검사, 실행 파이프라인으로 전달합니다.</p>

<div class="quick-grid">
  <a href="{{ '/getting-started/' | relative_url }}">
    <strong>Getting Started</strong>
    <span>로컬 실행, 테스트, 대시보드 확인</span>
  </a>
  <a href="{{ '/architecture/' | relative_url }}">
    <strong>Architecture</strong>
    <span>ingestion부터 execution까지 전체 흐름</span>
  </a>
  <a href="{{ '/strategy/' | relative_url }}">
    <strong>Strategy</strong>
    <span>Strategy trait 구현 방식과 성능 계약</span>
  </a>
</div>

## Pipeline

```text
[Binance WebSocket] -> ingestion -> orderbook -> strategy -> risk -> execution
```

시스템은 async I/O와 동기 처리 스레드를 분리합니다. WebSocket 수신, REST 스냅샷 조회, 주문 실행은 Tokio 런타임에서 처리하고, 오더북 유지와 전략 평가는 별도 OS 스레드에서 수행합니다. 두 영역은 `crossbeam-channel`로 연결됩니다.

## Current Milestones

| Milestone | Status | Description |
| --- | --- | --- |
| M1 | Done | Project setup, WebSocket connection, raw stream output |
| M2 | Done | Orderbook sync with snapshot + diff, unit tests |
| M3 | Done | Multi-thread pipeline, channels, watchdog, graceful shutdown |
| M4 | Done | Risk checks and paper trading mode |
| M5 | Done | Backtesting harness |
| M6 | Planned | Strategy implementation and cloud VM deployment |

## Main Components

| Module | Role |
| --- | --- |
| `ingestion` | Binance depth stream 수신, REST snapshot 조회, reconnect 이벤트 발행 |
| `orderbook` | Snapshot + diff 기반 로컬 오더북 유지와 sequence gap 감지 |
| `strategy` | 오더북 snapshot을 받아 `Signal` 생성 |
| `risk` | 주문 수량, 잔고, 가격 밴드, 초당 주문 수 제한 |
| `execution` | Paper execution 및 주문 로그 기록 |
| `backtest` | 과거 depth update 파일 또는 합성 데이터 기반 PnL 검증 |
| `dashboard` | 실시간 상태, 가격 히스토리, halt/resume endpoint 제공 |

## Documentation Source

이 사이트는 repository의 `docs/` 폴더에 있는 Markdown 파일로 구성됩니다. GitHub Pages 설정에서 source를 `Deploy from a branch`, branch를 `main`, folder를 `/docs`로 선택하면 렌더링됩니다. repository 이름이 `rust-hft`가 아니거나 custom domain을 쓰는 경우 `docs/_config.yml`의 `baseurl`을 배포 경로에 맞게 바꿉니다.
