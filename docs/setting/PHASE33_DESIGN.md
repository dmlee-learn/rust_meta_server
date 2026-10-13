# Phase 33 설계: 프로덕션 강화

> Graceful shutdown + Deep health check + Config validation.

---

## 1. Graceful Shutdown

### D33-1: SIGTERM/SIGINT 처리
- `tokio::signal::ctrl_c` + SIGTERM listener
- 수신 시 "shutting down" 로그 + `axum::serve(...).with_graceful_shutdown(shutdown_signal())`
- in-flight 요청 완료 대기 후 종료

### D33-2: `with_graceful_shutdown`
- `axum::serve(listener, router).with_graceful_shutdown(async move { shutdown_signal().await })`
- 신호 수신 시 새 요청 거부, 기존 요청 완료

---

## 2. Deep Health Check

### D33-3: `/health/deep` 엔드포인트
- L3 DataStore 연결 확인 (SELECT 1)
- Redis 연결 확인 (PING) — enabled일 때
- L1 cache 상태
- 엔진 상태
- 각 컴포넌트 status: ok/degraded

### D33-4: `/health` (기본) vs `/health/deep`
- 기본: lightweight (engine/socket 상태만)
- deep: DB/Redis 실제 연결 확인

---

## 3. Config Validation

### D33-5: Startup config 검증
- `main.rs`에서 환경변수 검증:
  - `DATABASE_BACKEND` 유효값 확인 (memory/postgres)
  - `JWT_SECRET` 길이 (disabled 아니면 32자+)
  - `RATE_LIMIT_PER_MIN` 음수 없음
  - `REDIS_ENABLED` 유효값
- 잘못된 값 → 명확한 에러 메시지 + exit

---

## 4. 파일 변경

| 파일 | 변경 |
|------|------|
| `src/main.rs` | graceful shutdown + config validation |
| `src/http.rs` | `/health/deep` 엔드포인트 |

---

## 5. 검증

- `/health/deep` Docker 실 검증
- config validation: 잘못된 값 시 에러 확인
- graceful shutdown: SIGTERM 시 log 확인
- 전체 테스트 통과
