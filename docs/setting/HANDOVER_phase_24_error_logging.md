# HANDOVER: Phase 24 — 에러 로그 기록 + Metrics 연동

> 이 문서는 Phase 24를 완료하고 단절했을 때, 다음 개발자(또는 다음 세션의 AI 에이전트)가
> 이어서 작업을 진행할 수 있도록 작성된 핸드오버 문서입니다.

---

## 1. Phase 24 완료 내용

### 1.1 에러 로그 기록
- `AppState::record_error(code, message, domain)`: 에러 로그 링 버퍼에 기록
- `board_create`: 에러 발생 시 `record_error()` + `metrics.record_hook_error()` 호출
- `handle_domain`: 에러 발생 시 `record_error()` + `metrics.record_hook_error()` 호출
- CR0005 (파이프라인 미기동)도 에러 로그에 기록

### 1.2 기록되는 에러 코드
- CJ1101: 구문 오류
- CJ1102: CPU Timeout
- CJ1103: 메모리 초과
- CJ1104: 비즈니스 예외
- CR0001: 메모리 부족
- CR0005: 파이프라인 미기동

### 1.3 Docker 실 검증
- 정상 요청 → 에러 없음
- 빈 title → `CJ1104` 에러 기록 (board)
- 잘못된 형식 → `CJ1104` 에러 기록 (erp)
- `/api/admin/logs`: 2개 에러 확인
- `/api/admin/metrics`: `hook errors: 2`
- 컨테이너 healthy

### 1.4 테스트
- 전체 96개 테스트 통과

---

## 2. 주의사항

### 2.1 에러 로그 용량
- 최대 100개 유지 (링 버퍼)
- 초과 시 가장 오래된 로그 제거
- 대시보드에서 마지막 50개 표시

### 2.2 Hook Error Metrics
- `metrics.record_hook_error()`는 에러 발생 시마다 호출
- `/api/admin/metrics`에서 `hooks.errors`로 확인
- Prometheus `/metrics`에서 `meta_server_hook_errors_total`로 확인

### 2.3 에러 기록 위치
- HTTP 핸들러 레벨에서 기록 (pipeline 내부가 아닌)
- pipeline 내부에서 `map_infra_error`가 에러 코드를 생성
- 핸들러가 그 코드를 받아서 로그에 기록

---

## 3. 다음 단계 (Phase 25)

### 3.1 성능 최적화
- Connection pooling (PostgreSQL, Redis)
- Query optimization
- Cache warming

### 3.2 보안 강화
- Admin API 인증 (admin token)
- CORS 정책
- CSRF protection

### 3.3 대시보드 추가 기능
- 실시간 히트맵
- 서버 제어 (restart, health check)
- 알림 (에러 발생 시 브라우저 알림)

### 3.4 테스트
- Integration test suite
- Load testing
- Chaos engineering

---

## 4. 빌드 및 테스트

```bash
# 전체 테스트 (96개)
cargo test

# Docker Compose 기동
cd /mnt/c/ai/meta_server
docker compose up -d

# 정상 요청
curl -X POST http://localhost:3000/api/board \
  -H "Content-Type: application/json" \
  -d '{"title":"Normal","body":"OK"}'

# 에러 유발 요청
curl -X POST http://localhost:3000/api/board \
  -H "Content-Type: application/json" \
  -d '{"title":"","body":"Empty"}'

# 에러 로그 확인
curl http://localhost:3000/api/admin/logs

# 메트릭스 확인
curl http://localhost:3000/api/admin/metrics

# 대시보드 확인
# http://localhost:3000/admin

# Docker Compose 종료
docker compose down
```
