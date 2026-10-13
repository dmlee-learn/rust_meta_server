# Phase 29 설계: Integration Test Suite

> E2E 테스트 + DB 상태 검증 + k6 load test 스크립트.

---

## 1. Integration Test (`tests/phase29_integration.rs`)

### D29-1: E2E Board Create + DB 검증
- MemoryStore 기반 파이프라인 기동
- `handle_create`로 정상 글 생성
- DataStore에서 저장된 글 조회 확인
- L1 cache에 적재 확인

### D29-2: E2E Domain Create (ERP) + 검증
- DomainPipeline(erp) 기동
- `handle_request`로 발주서 생성
- 저장 확인

### D29-3: Input Validation 통합 테스트
- 빈 title → CJ1201 (422)
- 음수 quantity (ERP) → CJ1201 (422)
- 정상 → 200

### D29-4: Cache Invalidation 테스트
- 모듈 로드 → L1 적재 확인
- invalidate → L1에서 제거 확인
- 재로드 → L1 재적재 확인

### D29-5: Error Log 기록 테스트
- 에러 유발 요청 → error_logs에 기록 확인
- hook_errors metrics 증가 확인

---

## 2. k6 Load Test 스크립트

### D29-6: `loadtest/k6.js`
- board/erp/blog/shop endpoint에 대한 부하 테스트
- Virtual Users: 100 → 500 (staged)
- Duration: 30s each stage
- Metrics: p95/p99 latency, throughput, error rate
- Health endpoint warm-up

### D29-7: `loadtest/README.md`
- k6 설치 방법
- 실행 명령
- 결과 해석

---

## 3. 파일 변경

| 파일 | 변경 |
|------|------|
| `tests/phase29_integration.rs` | 신규 integration test |
| `loadtest/k6.js` | 신규 load test 스크립트 |
| `loadtest/README.md` | load test 가이드 |

---

## 4. 검증

- `cargo test --test phase29_integration` 통과
- 기존 전체 테스트 통과
- k6 스크립트 구문 확인 (k6 없어도 파일 존재 확인)
