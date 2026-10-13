# HANDOVER: Phase 29 — Integration Test Suite

> 이 문서는 Phase 29를 완료하고 단절했을 때, 다음 개발자(또는 다음 세션의 AI 에이전트)가
> 이어서 작업을 진행할 수 있도록 작성된 핸드오버 문서입니다.

---

## 1. Phase 29 완료 내용

### 1.1 Integration Test (`tests/phase29_integration.rs`)
5개 E2E 테스트 (MemoryStore 기반):
- `e2e_board_create_and_db_verify`: Board 생성 → `get_board_post` DB 검증 + L1 적재
- `e2e_erp_create_and_db_verify`: ERP 발주서 생성 → `get_erp_purchase_order` DB 검증
- `input_validation_empty_title`: 빈 title → CJ1201 (422)
- `input_validation_negative_quantity_erp`: 음수 quantity → CJ1201 (422)
- `cache_invalidation_cycle`: 적재 → 무효화 → 재로드

### 1.2 k6 Load Test 스크립트
- `loadtest/k6.js`: 100→500 VUs, board/erp/blog/shop 혼합 부하
- Thresholds: p95<500ms, p99<1000ms, error rate<5%
- `loadtest/README.md`: 설치/실행/해석 가이드

### 1.3 테스트
- 전체 101개 통과 (96 기존 + 5 신규 integration)

---

## 2. 주의사항

### 2.1 Integration test 범위
- MemoryStore 기반 (DB 연결 없음) → 빠르고 안정적
- PostgreSQL 기반 E2E는 Docker Compose 수동 검증 필요
- `DomainPipeline::build`가 base/ext 모듈을 자체 upsert (fixture에서 중복 upsert 불필요)

### 2.2 k6 load test
- k6 설치 필요 (외부 도구)
- 실 환경에서 실행: `META_SERVER_URL=http://localhost:3000 k6 run loadtest/k6.js`
- rate limiting (100/min) → 429 응답 가능성 (load test 시 RATE_LIMIT_PER_MIN 상향 권장)

### 2.3 Rate limit과 load test 충돌
- k6 500 VUs는 rate limit(100/min) 초과 → 429 다수
- load test 시 `RATE_LIMIT_PER_MIN` 환경변수 상향 (예: 10000)

---

## 3. 다음 단계 (Phase 30)

### 3.1 Chaos Engineering
- PostgreSQL 일시 장애 시나리오 (connection drop)
- Redis 장애 시나리오 (fallback to L1)
- OOM 시나리오
- Recovery time 측정

### 3.2 배포 자동화
- GitHub Actions CI/CD 파이프라인
- Docker image build + push
- Health check 기반 rolling deploy
- Rollback 전략

---

## 4. 빌드 및 테스트

```bash
# 전체 테스트 (101개)
cargo test

# integration test만
cargo test --test phase29_integration

# k6 load test (k6 설치 필요)
META_SERVER_URL=http://localhost:3000 k6 run loadtest/k6.js
```
