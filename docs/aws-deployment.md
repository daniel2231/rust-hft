---
title: AWS Deployment
---

# AWS 배포 및 고도화 로드맵

Phase 2(실운영)를 위한 AWS 세팅 가이드입니다. 처음부터 모든 최적화를 적용하는 것이 아니라, **"기본 세팅 → 측정 → 병목이 확인된 것만 고도화"** 순서로 단계를 나눕니다. 각 단계는 이전 단계가 안정적으로 돌아가고, 측정 데이터가 다음 단계의 필요성을 뒷받침할 때만 진행하세요.

## 왜 AWS 도쿄인가

Binance 선물 매칭 엔진은 AWS ap-northeast-1(도쿄) 리전에 있습니다. 같은 리전에 배포하면 네트워크 RTT가 로컬 대비 10~40배 줄어듭니다.

| 환경 | RTT | 용도 |
|------|-----|------|
| Mac Mini 로컬 (Phase 1) | ~40–80ms | 기능 개발, 정확성 검증 |
| AWS 도쿄 (Phase 2) | ~2–5ms | 실운영 |

> 전략이 레이턴시에 민감하지 않다면(수 초~분 단위 보유) 이 이점은 크지 않습니다. 자신의 전략에 레이턴시가 실제로 중요한지부터 판단하세요.

---

## Stage 1 — 기본 세팅 (Paper 모드 검증)

목표: 도쿄 리전 EC2에서 systemd 서비스로 Paper 모드를 안정적으로 돌리는 것.

### 1. EC2 인스턴스 생성

| 항목 | 권장 | 이유 |
|------|------|------|
| 리전 | `ap-northeast-1` (도쿄) | Binance 매칭 엔진과 동일 리전 |
| 인스턴스 타입 | `c7g.medium` 또는 `c7i.large` | 컴퓨트 최적화. 스레드 2개 구조이므로 vCPU 2개면 충분 |
| **피해야 할 타입** | `t3`/`t4g` 등 버스터블 | CPU 크레딧 고갈 시 성능이 급락 — 레이턴시 지터의 주범 |
| AMI | Ubuntu 24.04 LTS 또는 Amazon Linux 2023 | systemd 지원, 장기 지원 |
| 스토리지 | gp3 20GB | 로그·바이너리용. IO 성능은 중요하지 않음 (핫 패스에 디스크 IO 없음) |

> `c7g`(ARM Graviton)는 같은 성능에 더 저렴하지만, Rust 크로스 컴파일이 아닌 인스턴스 위에서 직접 빌드한다면 아키텍처를 신경 쓸 필요가 없습니다.

### 2. 보안 그룹 (방화벽)

**인바운드는 SSH만 엽니다.**

| 방향 | 포트 | 소스 | 용도 |
|------|------|------|------|
| 인바운드 | 22 | 내 IP만 | SSH |
| 인바운드 | ~~3000~~ | **열지 않음** | 대시보드는 인증이 없음 — SSH 터널로만 접근 |
| 아웃바운드 | 443 | 전체 | Binance WebSocket/REST |

대시보드 접근은 SSH 터널로:

```bash
ssh -L 3000:localhost:3000 ubuntu@<EC2-IP>
# 로컬 브라우저에서 http://localhost:3000
```

### 3. 서버 초기 설정 및 배포

```bash
# SSH 접속 후
sudo apt update && sudo apt install -y build-essential pkg-config git
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh   # Rust 설치
source "$HOME/.cargo/env"

git clone https://github.com/daniel2231/rust-hft.git
cd rust-hft
sudo deploy/install.sh        # 빌드 + systemd 등록 (production.md 참조)
sudo systemctl start crypto-trader
journalctl -u crypto-trader -f
```

이후 서비스 관리·라이브 전환 절차는 [실전 운영](production.md)과 동일합니다.

### 4. Stage 1 완료 기준

- [ ] Paper 모드로 24시간 이상 무중단 실행 (watchdog 재연결 포함 정상 동작)
- [ ] `journalctl`에서 시퀀스 갭/재동기화 빈도 확인 — 잦다면 네트워크나 인스턴스 문제
- [ ] 이벤트 처리 로그(1000 이벤트 단위)가 꾸준히 올라오는지 확인
- [ ] 킬 스위치 원격 동작 확인 (`sudo touch /opt/crypto-trader/HALT`)

---

## Stage 2 — 운영 안정화 (라이브 전환 전후)

목표: 사람이 안 보고 있어도 문제를 알 수 있게 만드는 것. **성능 튜닝보다 먼저입니다.**

### 모니터링·알림

- **CloudWatch Agent**로 CPU/메모리/네트워크 지표 수집. 특히 다음에 알람 설정:
  - 인스턴스 상태 체크 실패 (StatusCheckFailed)
  - CPU 크레딧(버스터블을 썼다면) 또는 CPU 사용률 급변
- **프로세스 생존 알림**: systemd 재시작이 반복되면 알아야 합니다. 간단하게는
  `journalctl -u crypto-trader --since "5 min ago" | grep -c "starting"`을 cron으로 검사해
  SNS/슬랙 웹훅으로 알림을 보내는 스크립트면 충분합니다.
- **로그 보존**: journald 기본 설정은 용량 제한이 있습니다. 장기 보존이 필요하면
  CloudWatch Logs로 포워딩하거나 `/etc/systemd/journald.conf`에서 `SystemMaxUse`를 조정하세요.

### 자격증명·네트워크 보안

- Binance API 키에 **IP 화이트리스트**로 EC2의 Elastic IP를 등록 (Elastic IP를 먼저 할당해 IP를 고정).
- 출금 권한은 반드시 비활성화.
- SSH는 키 인증만 허용, 가능하면 SSM Session Manager로 전환해 22번 포트도 닫기.

### 비용 관리

- 1대 상시 운영이므로 **Savings Plan / Reserved Instance**(1년 약정)로 30~40% 절감 가능.
- **스팟 인스턴스는 사용 금지** — 회수되면 포지션을 둔 채 프로세스가 사라집니다.

### Stage 2 완료 기준

- [ ] 인스턴스/프로세스 장애 시 5분 내 알림 수신
- [ ] Elastic IP 고정 + Binance IP 화이트리스트 적용
- [ ] 소액 라이브로 1주 이상 무사고 운영

---

## Stage 3 — 레이턴시 측정 (튜닝 전 필수)

목표: **무엇이 느린지 숫자로 확인.** 측정 없이 Stage 4로 넘어가지 마세요.

### 측정할 것

| 지표 | 방법 |
|------|------|
| 네트워크 RTT | `ping fstream.binance.com`, WebSocket 이벤트 수신 타임스탬프 vs 이벤트의 `E` 필드 차이 |
| 파이프라인 내부 지연 | `src/metrics.rs`의 `LatencyTracker`를 확장해 수신→전략→리스크 각 구간 계측 |
| 지터(tail latency) | 평균이 아니라 p99/p999를 보세요. HFT에서 문제는 평균이 아니라 꼬리입니다 |
| 채널 백로그 | 로그의 `channel backlog exceeds 100 items` 경고 빈도 |

> 계측 코드를 추가할 때 새 타이밍 메커니즘을 만들지 말고 기존 `LatencyTracker`를 확장하세요 (`CLAUDE.md` 참조). 계측 자체가 핫 패스에 할당이나 syscall을 추가하면 안 됩니다.

### 판단 기준

- 내부 파이프라인 지연이 수 마이크로초, 네트워크가 수 밀리초라면 → **병목은 네트워크.** 코드 튜닝(Stage 4)보다 배치 최적화(Stage 5)가 유효.
- p99 지터가 크고 원인이 스케줄링이면 → Stage 4의 CPU pinning이 유효.
- 채널 백로그 경고가 잦으면 → 전략 코드의 처리 시간부터 프로파일링.

---

## Stage 4 — 인스턴스·OS 레벨 튜닝

목표: 측정에서 확인된 지터/스케줄링 병목 제거. **Stage 3의 측정 데이터가 있을 때만 진행.**

### CPU pinning (코드 변경)

스케줄러 유발 지터(캐시 미스, 비일관적 tail latency)가 측정되면, 오더북/전략 스레드를
[`core_affinity`](https://crates.io/crates/core_affinity) 크레이트로 특정 코어에 고정합니다.

- 먼저 `lscpu -e`로 코어 레이아웃 확인 — **물리 코어와 하이퍼스레드 시블링 구분이 중요**합니다. 핫 스레드와 다른 스레드가 같은 물리 코어의 HT 시블링을 공유하면 pinning 효과가 사라집니다.
- 오더북/전략 스레드(핫)를 한 물리 코어에, Tokio 런타임을 나머지에 배치.
- PR에 측정 결과(전/후 p99)를 함께 기록하세요.

### OS 튜닝 (인스턴스 설정)

측정으로 뒷받침될 때 순서대로:

1. **인스턴스 업그레이드**: vCPU 4개 이상(`c7i.xlarge`)으로 올려 핫 스레드에 코어를 여유 있게 배정.
2. **`isolcpus` / `nohz_full`** 커널 파라미터로 핫 코어를 OS 스케줄러에서 격리 (GRUB 설정, 재부팅 필요).
3. **CPU governor**를 `performance`로 고정 (주파수 스케일링 지터 제거).
4. **ENA 네트워크**: 최신 세대 인스턴스는 기본 활성화. `ethtool -S`로 드롭 여부만 확인.

### 하지 말 것 (이 규모에서)

- SPSC 링 버퍼, 전용 로깅 스레드 등 새 스레드/채널 추가 — 측정된 병목 없이 추가하면 복잡도만 늘어납니다 (`CLAUDE.md`의 스레딩 규칙).
- DPDK/커널 바이패스, 베어메탈 인스턴스 — 클라우드 crypto HFT에서 네트워크 RTT(ms 단위)가 지배하는 한 과잉 투자입니다.

---

## Stage 5 — 배치·아키텍처 고도화

목표: 네트워크가 병목으로 확인됐고 전략 수익이 인프라 투자를 정당화할 때.

- **가용 영역(AZ) 선정**: Binance 엔드포인트까지의 RTT는 AZ마다 다를 수 있습니다. 여러 AZ에 임시 인스턴스를 띄워 RTT를 비교하고 가장 낮은 AZ를 선택하세요.
- **클러스터 배치 그룹(Cluster Placement Group)**: 자체 컴포넌트가 여러 인스턴스로 분리될 때만 의미 있음. 단일 인스턴스 구조인 현재는 불필요.
- **멀티 심볼 확장**: 심볼이 늘어 이벤트 레이트가 오르면 먼저 단일 오더북 스레드가 감당 가능한지 측정 → 초과 시 심볼 샤딩(심볼 그룹별 스레드) 검토. 이때 비로소 `CLAUDE.md`가 언급하는 SPSC 큐가 후보가 됩니다.
- **장애 복구(DR)**: 두 번째 인스턴스를 상시 대기시키기보다, ① 킬 스위치 → ② AMI 스냅샷에서 재기동하는 런북을 문서화하는 것이 단일 전략 규모에는 현실적입니다. 자동 페일오버는 중복 주문 위험(두 인스턴스가 동시에 주문)이 있으므로 신중하게.

---

## 단계 요약

| Stage | 시점 | 핵심 작업 | 진행 조건 |
|-------|------|-----------|-----------|
| 1. 기본 세팅 | Phase 2 시작 | 도쿄 EC2 + systemd + Paper | — |
| 2. 운영 안정화 | 라이브 전환 전후 | 모니터링, 알림, IP 고정, 비용 | Stage 1 안정 |
| 3. 측정 | 튜닝 필요성 검토 시 | RTT/파이프라인/지터 계측 | Stage 2 완료 |
| 4. OS/인스턴스 튜닝 | 지터가 측정으로 확인됨 | CPU pinning, isolcpus, governor | Stage 3 데이터 |
| 5. 배치 고도화 | 네트워크가 병목 + 수익 정당화 | AZ 선정, 멀티 심볼 샤딩, DR | Stage 3~4 데이터 |

관련 문서: [실전 운영](production.md) (systemd 배포·서비스 관리), [설정](configuration.md), 리포지토리 루트 `CLAUDE.md` (핫 패스·스레딩 원칙).
