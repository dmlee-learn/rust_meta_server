# HANDOVER: Phase 19 — User-based Rate Limit + JWT Sub 추출

> 이 문서는 Phase 19를 완료하고 단절했을 때, 다음 개발자(또는 다음 세션의 AI 에이전트)가
> 이어서 작업을 진행할 수 있도록 작성된 핸드오버 문서입니다.

---

## 1. Phase 19 완료 내용

### 1.1 User-based Rate Limit
- `RateLimitConfig`에 `user_per_minute: u32` 추가 (기본 60)
- `RateLimiter::check_user()` — user별 요청 체크
- `extract_user_from_request()` — JWT sub 추출
- `decode_token_claims()` — auth.rs에 추가 (env secret 사용)
- 에러 코드: `CR4003` (user rate limit)

### 1.2 Rate Limiting 계층
- **Global**: IP별 분당 `RATE_LIMIT_PER_MIN` (기본 60)
- **Domain**: IP + domain별 분당 `RATE_LIMIT_{DOMAIN}_PER_MIN`
- **User**: JWT sub별 분당 `RATE_LIMIT_USER_PER_MIN` (기본 60)

### 1.3 Docker Compose 실 검증
- 3개 컨테이너 모두 healthy
- PostgreSQL INSERT 확인: `Phase 19 Test` (board) + `Acme` (erp)

### 1.4 테스트
- 전체 96개 테스트 통과

---

## 2. 주의사항

### 2.1 RateLimitConfig 변경
`RateLimitConfig`에 `user_per_minute` 필드가 추가되었습니다.
기존 코드에서 `RateLimitConfig { per_minute: N, per_domain: HashMap::new() }`을 사용한다면
`RateLimitConfig { per_minute: N, per_domain: HashMap::new(), user_per_minute: 60 }`로 수정해야 합니다.

### 2.2 JWT Secret
User-based rate limit은 JWT secret을 사용해야 합니다.
`JWT_SECRET` 환경변수를 설정해야 합니다:

```bash
export JWT_SECRET="my-secret-key"
```

### 2.3 Rate Limiting 순서
1. Global (IP) check
2. Domain (IP + domain) check
3. User (JWT sub) check

각 단계에서 제한 초과 시 해당 에러 코드 반환:
- `CR4001`: Global limit
- `CR4002`: Domain limit
- `CR4003`: User limit

---

## 3. 다음 단계 (Phase 20)

### 3.1 rquickjs C 라이브러리 시그널 문제 해결
- 별도 쓰레드에서 JS eval 실행
- rquickjs 업그레이드 확인

### 3.2 WebSocket 확장
- domain별 topic
- real-time 유효화 알림

### 3.3 Monitoring Dashboard
- Grafana dashboard
- Prometheus scraping 설정

### 3.4 API Documentation
- OpenAPI/Swagger spec
- API test suite

---

## 4. 빌드 및 테스트

```bash
# 전체 테스트 (96개)
cargo test

# Docker Compose 기동
cd /mnt/c/ai/meta_server
docker compose up -d

# User rate limit 테스트 (JWT 필요)
# 1. JWT 생성
# 2. Authorization: Bearer <token> 헤더로 요청
# 3. 61번째 요청은 429 CR4003 반환

# Docker Compose 종료
docker compose down
```
