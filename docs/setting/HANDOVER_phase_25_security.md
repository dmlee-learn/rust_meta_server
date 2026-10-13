# HANDOVER: Phase 25 — 보안 강화 (Admin 인증 + CORS + Input Validation)

> 이 문서는 Phase 25를 완료하고 단절했을 때, 다음 개발자(또는 다음 세션의 AI 에이전트)가
> 이어서 작업을 진행할 수 있도록 작성된 핸드오버 문서입니다.

---

## 1. Phase 25 완료 내용

### 1.1 Admin API 인증
- `ADMIN_TOKEN` 환경변수 설정 시 `/api/admin/*`에 `X-Admin-Token` 헤더 요구
- 미설정 시 (개발 환경) 인증 없이 접근 가능
- 에러 코드: `CR6001` (401 Unauthorized)
- Docker Compose: `ADMIN_TOKEN: "admin-secret-123"`

### 1.2 CORS 정책
- `CORS_ORIGINS` 환경변수 (쉼표 구분 origin 목록)
- 미설정 시: `*` (전부 허용)
- 허용 Methods: GET, POST, PUT, DELETE, OPTIONS
- 허용 Headers: Content-Type, Authorization, X-Admin-Token
- `Access-Control-Max-Age: 86400`
- Docker Compose: `CORS_ORIGINS: "http://localhost:3000,http://localhost:3001"`

### 1.3 Input Validation
- 에러 코드: `CJ1201` (422 Unprocessable Entity)
- Board: title 1~200자, body 0~10000자
- ERP: workflow_id 1~100자, supplier 1~200자, quantity 1~1000000
- Blog: title 1~200자, category 1~50자
- Shop: product_id 1~100자, quantity 1~1000000, price >= 0

### 1.4 Docker 실 검증
- Admin API without token → 401 `CR6001`
- Admin API with token → 200 정상
- CORS preflight → 204 + correct headers
- CORS actual request → `access-control-allow-origin` 헤더
- Empty title → `CJ1201`
- Negative quantity → `CJ1201`
- Normal request → OK
- 컨테이너 healthy

### 1.5 테스트
- 전체 96개 테스트 통과
- `board_create_reject_empty_title`: CJ1104 → CJ1201 (400 → 422) 업데이트

---

## 2. 주의사항

### 2.1 Admin Token
- Docker Compose에서 `ADMIN_TOKEN: "admin-secret-123"` 설정
- 대시보드에서 API 호출 시 `X-Admin-Token: admin-secret-123` 헤더 필요
- 또는: `ADMIN_TOKEN` 미설정 시 인증 없이 접근 가능 (개발용)

### 2.2 CORS
- `CORS_ORIGINS` 미설정 시 `*` (전부 허용)
- 프로덕션에서는 특정 origin만 지정
- 대시보드(3001)에서 API(3000) 호출 시 CORS 필요

### 2.3 Input Validation
- `CJ1201`은 422 상태 코드 반환
- 기존 `CJ1104`(비즈니스 예외)와 구분
- Validation은 pipeline 내부가 아닌 HTTP 핸들러 레벨에서 수행

---

## 3. 다음 단계 (Phase 26)

### 3.1 성능 최적화
- Connection pooling (PostgreSQL, Redis)
- Query optimization
- Cache warming

### 3.2 대시보드 추가 기능
- 실시간 히트맵
- 서버 제어 (restart, health check)
- 알림 (에러 발생 시 브라우저 알림)

### 3.3 테스트
- Integration test suite
- Load testing
- Chaos engineering

### 3.4 API Documentation
- OpenAPI spec 업데이트 (CJ1201, CR6001 추가)
- API usage examples

---

## 4. 빌드 및 테스트

```bash
# 전체 테스트 (96개)
cargo test

# Docker Compose 기동
cd /mnt/c/ai/meta_server
docker compose up -d

# Admin API (token 필요)
curl http://localhost:3000/api/admin/status \
  -H "X-Admin-Token: admin-secret-123"

# CORS 테스트
curl -X OPTIONS http://localhost:3000/api/board \
  -H "Origin: http://localhost:3001" \
  -H "Access-Control-Request-Method: POST"

# Input Validation 테스트
curl -X POST http://localhost:3000/api/board \
  -H "Content-Type: application/json" \
  -d '{"title":"","body":"Test"}'

# Docker Compose 종료
docker compose down
```
