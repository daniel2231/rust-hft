# 실전 운영

리눅스 서버에서 24/7로 운영하는 방법입니다. **systemd 서비스**로 등록하면 크래시 시 자동 재시작, 부팅 시 자동 시작, journald 로깅을 얻습니다.

## 배포 전 체크리스트

1. ✅ 백테스트에서 전략 PnL 검증 완료 ([백테스팅](backtesting.md))
2. ✅ Paper 모드에서 실시간으로 충분히 검증 완료 (신호 빈도, 리스크 거부율 확인)
3. ✅ `config/default.toml`의 `[risk]` 한도를 실제 자본 규모에 맞게 설정
4. ✅ 킬 스위치 동작 확인 (`touch HALT` → 주문 차단 로그 확인)

## 자동 설치 (systemd)

서버에서 레포 루트 기준으로 실행:

```bash
sudo deploy/install.sh
```

스크립트가 수행하는 작업:

1. `cargo build --release`로 바이너리 빌드 (sudo를 호출한 원래 유저 권한으로 빌드)
2. 전용 시스템 유저 `crypto` 생성 (로그인 불가, 홈 없음)
3. `/opt/crypto-trader`에 바이너리·`config/`·`.env` 설치 (`.env`는 권한 `0600`, 기존 파일은 덮어쓰지 않음)
4. systemd 유닛 등록 및 부팅 시 자동 시작 활성화

## 서비스 관리

```bash
sudo systemctl start crypto-trader      # 시작
sudo systemctl status crypto-trader     # 상태 확인
sudo systemctl restart crypto-trader    # 재시작
sudo systemctl stop crypto-trader       # 중단 (SIGINT → 그레이스풀 셧다운)
journalctl -u crypto-trader -f          # 실시간 로그
journalctl -u crypto-trader --since "1 hour ago"   # 최근 1시간 로그
```

로그 레벨, 재시작 정책, 하드닝 옵션은 유닛 파일 [`deploy/crypto-trader.service`](../deploy/crypto-trader.service)에서 조정합니다.

## 라이브 모드 전환

> ⚠️ 라이브 모드(실주문)는 M6 마일스톤 이후 지원 예정입니다. 아래는 전환 절차입니다.

1. API 자격증명 입력:

```bash
sudo nano /opt/crypto-trader/.env       # BINANCE_API_KEY / BINANCE_API_SECRET
```

2. 모드 변경:

```bash
sudo nano /opt/crypto-trader/config/default.toml   # mode = "live"
```

3. 재시작:

```bash
sudo systemctl restart crypto-trader
```

### API 키 보안

- `.env`는 절대 git에 커밋하지 않습니다 (`.gitignore`에 포함됨).
- 설치 스크립트가 `.env`를 `0600` 권한으로 유지합니다.
- Binance API 키에는 **거래 권한만** 부여하고 출금 권한은 끄세요. 가능하면 서버 IP 화이트리스트를 설정하세요.
- Paper 모드는 거래소 API를 아예 호출하지 않으므로 키가 필요 없습니다.

## 운영 중 긴급 중단 (킬 스위치)

```bash
sudo touch /opt/crypto-trader/HALT      # 즉시 모든 주문 차단 (프로세스는 계속 실행)
sudo rm /opt/crypto-trader/HALT         # 주문 재개
```

데이터 수신과 오더북 유지는 계속되고 주문만 차단되므로, 재개 시 재동기화 없이 바로 복귀합니다. 프로세스 자체를 내려야 하면 `sudo systemctl stop crypto-trader`를 사용하세요 (그레이스풀 셧다운).

## 대시보드 접근

서버의 대시보드(포트 3000)에는 **인증이 없으므로** 공개 인터넷에 노출하지 마세요. SSH 터널로 접근합니다:

```bash
ssh -L 3000:localhost:3000 user@server
# 로컬 브라우저에서 http://localhost:3000
```

필요 없다면 `[dashboard] enabled = false`로 꺼두는 것이 가장 안전합니다.

## 안정성 메커니즘 요약

| 메커니즘 | 동작 |
|----------|------|
| systemd `Restart` | 프로세스 크래시 시 자동 재시작 |
| Watchdog | 30초(설정 가능)간 메시지 없으면 WebSocket 자동 재연결 |
| 시퀀스 갭 감지 | 오더북 update ID 갭 발견 시 스냅샷 재동기화 |
| 킬 스위치 | `HALT` 파일 존재 시 모든 주문 차단 |
| 리스크 체크 | 수량/잔고/가격 밴드/주문 빈도/중복 ID 검사 ([설정](configuration.md)) |

## 인프라 로드맵

| Phase | 환경 | RTT | 목적 |
|-------|------|-----|------|
| Phase 1 (현재) | Mac Mini 로컬 | ~40–80ms | 기능 개발 및 정확성 검증 |
| Phase 2 | AWS ap-northeast-1 (도쿄) | ~2–5ms | 전략 구현 후 실운영 |

Binance 선물 매칭 엔진이 도쿄 리전에 있으므로, 레이턴시가 중요해지는 시점에 도쿄 리전 VM으로 이전합니다.
