# HANDOVER: Phase 33 — 프로덕션 강화

> 이 문서는 Phase 33을 완료하고 단절했을 때, 다음 개발자(또는 다음 세션의 AI 에이전트)가
> 이어서 작업을 진행할 수 있도록 작성된 핸드오버 문서입니다.

---

## 1. Phase 33 완료 내용

### 1.1 Graceful Shutdown (`src/main.rs`)
- `shutdown_signal()`: SIGINT + SIGTERM listener (unix/non-unix branch)
- `axum::serve(listener, router).with_graceful_shutdown(shutdown_signal())`
- 신호 수신 시 in-flight 요청 완료 후 종료
- 로그: `SIGTERM received, initiating graceful shutdown` → `graceful shutdown complete`

### 1.2 Config Validation (`src/main.rs`)
- `validate_config()`: 기동 전 환경변수 검증
- `DATABASE_BACKEND` ∈ {memory, postgres} (위반 → CR0004)
- `REDIS_ENABLED` ∈ {true, false, 1, 0} (위반 → CR0005)
- `JWT_SECRET` 길이 < 32 → warning (disabled 아니면)
- `RATE_LIMIT_PER_MIN` < 0 → CR0006

### 1.3 Deep Health Check (`src/http.rs`)
- `GET /health/deep`: L3 DB 연결 (`list_enabled_modules`) + L1 cache + engine 상태
- 기본 `/health`(lightweight)와 구분

### 1.4 Docker 실 검증
- `/health/deep`: `{"database":{"modules_loaded":8,"status":"ok"},"cache_l1_modules":8,...}`
- SIGTERM: `graceful shutdown complete`
- 잘못된 config: `CR0004: DATABASE_BACKEND='invalid' is invalid`

### 1.5 테스트
- 전체 104개 통과

---

## 2. 주의사항

### 2.1 Unix branch 컴파일
- `#[cfg(unix)]` SIGTERM 코드는 Windows에서는 컴파일되지 않음
- Docker(Linux) 빌드에서만 검증 가능
- `tokio::signal::unix::SignalKind::terminate()` receiver는 `mut` 필요

### 2.2 Deep health check 한계
- L3 DB 확인은 `list_enabled_modules` (모듈 목록 조회) 사용
- Redis는 `enabled` flag만 보고, 실제 PING 없음 (RedisLikeStore에 ping 없음)
- Redis PING 필요 시 `RedisLikeStore` trait에 `ping()` 추가 필요

### 2.3 Graceful shutdown
- 새 요청 거부, in-flight 요청 대기
- Docker `stop` = SIGTERM → 정상 종료 확인
- timeout (기본 10s) 후 강제 종료 (Docker)

### 2.4 Config validation
- 기동 전 실행 → 잘못된 값이면 즉시 종료 (에러 코드 포함)
- JWT_SECRET은 warning만 (error 아님) — dev 환경 호환성

---

## 3. 다음 단계 (Phase 34)

### 3.1 성능 최적화 (고급)
- rquickjs context warm-up pool (pre-allocated)
- Zero-copy serialization
- Query optimization

### 3.2 Observability (고급)
- OpenTelemetry distributed tracing
- Redis PING에 기반한 deep health check 확장
- SLO error budget tracking

---

## 4. 빌드 및 검증

```bash
# 전체 테스트 (104개)
cargo test

# Docker build (이 환경에서 — Phase 32 finding 참고)
docker build -t meta_server:latest .
docker compose up -d --force-recreate meta_server

# /health/deep 확인
curl -s http://localhost:3000/health/deep

# graceful shutdown 확인
docker compose stop meta_server
docker compose logs meta_server | grep graceful

# config validation 확인
docker run --rm -e DATABASE_BACKEND=invalid \
  --entrypoint "" meta_server:latest /usr/local/bin/meta_server
```
