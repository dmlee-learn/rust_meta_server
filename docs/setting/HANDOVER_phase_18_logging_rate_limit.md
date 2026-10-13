# HANDOVER: Phase 18 — JSON Logging + Per-domain Rate Limit

> 이 문서는 Phase 18을 완료하고 단절했을 때, 다음 개발자(또는 다음 세션의 AI 에이전트)가
> 이어서 작업을 진행할 수 있도록 작성된 핸드오버 문서입니다.

---

## 1. Phase 18 완료 내용

### 1.1 JSON Structured Logging
- `LoggingObserver::json()` — JSON 형식 로그 출력
- 필드: `timestamp`, `level`, `message`, `module_id`, `version`, `elapsed_ms`, `error_code`
- 사용: `Observability::new(Arc::new(LoggingObserver::json(500)), 500)`

### 1.2 Per-domain Rate Limit
- `RateLimitConfig`에 `per_domain: HashMap<String, u32>` 추가
- 환경변수: `RATE_LIMIT_BOARD_PER_MIN`, `RATE_LIMIT_ERP_PER_MIN`, `RATE_LIMIT_BLOG_PER_MIN`, `RATE_LIMIT_SHOP_PER_MIN`
- `RateLimiter::check_domain()` — domain별 요청 체크
- `rate_limit_middleware` — global + domain limit 모두 체크
- 에러 코드: `CR4001` (global), `CR4002` (domain)

### 1.3 Docker Compose 실 검증
- 3개 컨테이너 모두 healthy
- PostgreSQL INSERT 확인: `Phase 18 Test` board_posts에 저장
- Prometheus duration histogram 존재

### 1.4 테스트
- 전체 96개 테스트 통과

---

## 2. 주의사항

### 2.1 RateLimitConfig 변경
`RateLimitConfig`에 `per_domain` 필드가 추가되었습니다.
기존 코드에서 `RateLimitConfig { per_minute: N }`을 사용한다면
`RateLimitConfig { per_minute: N, per_domain: HashMap::new() }`로 수정해야 합니다.

### 2.2 Per-domain Rate Limit 활성화
기본값은 global rate limit과 동일합니다.
domain별로 설정하려면 환경변수를 설정해야 합니다:

```bash
export RATE_LIMIT_ERP_PER_MIN=30  # ERP는 분당 30회
```

### 2.3 JSON Logging 활성화
기본은 텍스트 로그입니다. JSON 로그를 사용하려면:

```rust
let obs = LoggingObserver::json(500);  // 500ms threshold
let observability = Observability::new(Arc::new(obs), 500);
```

---

## 3. 다음 단계 (Phase 19)

### 3.1 rquickjs C 라이브러리 시그널 문제 해결
- 별도 쓰레드에서 JS eval 실행
- rquickjs 업그레이드 확인

### 3.2 User-based Rate Limit
- JWT sub 기반 rate limit
- per-user throttling

### 3.3 WebSocket 확장
- domain별 topic 구독
- real-time 유효화 알림

### 3.4 Monitoring Dashboard
- Grafana dashboard
- Prometheus scraping 설정

---

## 4. 빌드 및 테스트

```bash
# 전체 테스트 (96개)
cargo test

# Docker Compose 기동
cd /mnt/c/ai/meta_server
docker compose up -d

# Per-domain rate limit 테스트
export RATE_LIMIT_BOARD_PER_MIN=5
curl -X POST http://localhost:3000/api/board \
  -H "Content-Type: application/json" \
  -d '{"title":"Test","body":"Body"}'
# 6번째 요청은 429 CR4002 반환

# Docker Compose 종료
docker compose down
```
