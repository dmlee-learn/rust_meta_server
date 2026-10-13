# HANDOVER: Phase 23 — 실시간 WebSocket + 에러 로그 + 서버 제어

> 이 문서는 Phase 23을 완료하고 단절했을 때, 다음 개발자(또는 다음 세션의 AI 에이전트)가
> 이어서 작업을 진행할 수 있도록 작성된 핸드오버 문서입니다.

---

## 1. Phase 23 완료 내용

### 1.1 실시간 WebSocket (`GET /ws/admin`)
- 5초 간격으로 metrics JSON push
- `ws://localhost:3000/ws/admin`
- 대시보드에서 WebSocket 우선 → 실패 시 5초 polling 폴백
- JWT/Rate Limit 미들웨어 제외

### 1.2 에러 로그
- `AppState.error_logs`: `Mutex<VecDeque<ErrorLogEntry>>` (최대 100개)
- `GET /api/admin/logs`: 마지막 50개 조회
- `ErrorLogEntry`: `{code, message, timestamp, domain}`
- 대시보드에 에러 로그 패널 추가 (실시간 갱신)

### 1.3 서버 제어
- `POST /api/admin/cache/flush`: 모든 L1 캐시 flush
  - board + erp + blog + shop (4개)
  - `{ "code": "OK", "flushed": 4, "message": "4개 캐시 플러시 완료" }`
- `GET /api/admin/ping`: 가벼운 생존 확인
  - `{ "code": "OK", "ts": 1791003434 }`

### 1.4 대시보드 업데이트
- WebSocket 연결 시 `POLLING` → `LIVE` 표시
- 에러 로그 패널: 코드, 도메인, 메시지, 타임스탬프
- 캐시 flush 버튼: `🗑️ Flush Cache`
- WebSocket 실패 시 자동 polling 폴백

### 1.5 Docker Compose 실 검증
- `GET /api/admin/ping` → `{"code":"OK","ts":...}`
- `GET /api/admin/logs` → `{"logs":[],"total":0}`
- `POST /api/admin/cache/flush` → `{"code":"OK","flushed":4}`
- `/admin` HTML에 `flushCache`, `logList`, `wsStatus`, `connectWebSocket` 확인
- `POST /api/board` → `{"code":"OK","id":1}`
- `GET /api/admin/metrics` → `board: 1`, `engine_memory: 0.107 MB`
- 컨테이너 healthy

### 1.6 테스트
- 전체 96개 테스트 통과

---

## 2. 주의사항

### 2.1 WebSocket vs Polling
- 대시보드는 WebSocket 우선으로 동작
- WebSocket 연결 실패 시 5초 polling으로 자동 전환
- 연결 상태는 우측 하단 푸터에 표시 (LIVE/POLLING)

### 2.2 에러 로그
- 현재 에러 로그는 빈 상태 (로그 기록 로직 미구현)
- Phase 24에서 훅 에러, 시스템 에러를 로그에 기록하도록 구현
- `record_error()` 메서드 추가 필요

### 2.3 캐시 Flush
- `POST /api/admin/cache/flush`는 모든 도메인 캐시를 즉시 flush
- 프로덕션에서는 주의해서 사용 (성능 영향)

### 2.4 `/ws/admin` 경로
- `/ws` (JS 훅)와 `/ws/admin` (대시보드)는 별도 엔드포인트
- `/ws/admin`은 세션 풀을 사용하지 않음 (간단 push 전용)

---

## 3. 다음 단계 (Phase 24)

### 3.1 에러 로그 기록
- 훅 에러 (CJ1104 등) 발생 시 `error_logs`에 기록
- 시스템 에러 (DB 연결 실패, Redis 연결 실패) 기록
- `record_error()` 메서드 구현 + pipeline/domain에 통합

### 3.2 성능 최적화
- Connection pooling (PostgreSQL, Redis)
- Query optimization
- Cache warming

### 3.3 보안 강화
- Admin API 인증 (admin token)
- CORS 정책
- CSRF protection

### 3.4 대시보드 추가 기능
- 실시간 히트맵
- 서버 제어 (restart, health check)
- 알림 (에러 발생 시 브라우저 알림)

---

## 4. 빌드 및 테스트

```bash
# 전체 테스트 (96개)
cargo test

# Docker Compose 기동
cd /mnt/c/ai/meta_server
docker compose up -d

# 대시보드 확인
# http://localhost:3000/admin

# API 호출
curl http://localhost:3000/api/admin/ping
curl http://localhost:3000/api/admin/logs
curl -X POST http://localhost:3000/api/admin/cache/flush
curl http://localhost:3000/api/admin/status
curl http://localhost:3000/api/admin/metrics

# WebSocket 테스트 (wscat 사용)
# wscat -c ws://localhost:3000/ws/admin

# Docker Compose 종료
docker compose down
```
