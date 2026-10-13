# HANDOVER: Phase 30 — Chaos Engineering

> 이 문서는 Phase 30를 완료하고 단절했을 때, 다음 개발자(또는 다음 세션의 AI 에이전트)가
> 이어서 작업을 진행할 수 있도록 작성된 핸드오버 문서입니다.

---

## 1. Phase 30 완료 내용

### 1.1 Chaos Test (`tests/phase30_chaos.rs`)
3개 fault-injection 테스트:
- `redis_down_falls_back_to_l3`: FailingRedis(L2 장애) → L1 miss 시 L3 로드 성공
- `recovery_after_invalidation`: invalidate → L1 비어 있음 → 재로드 → L1 적재
- `save_failure_returns_error_not_panic`: FailingStore(save 실패) → 에러 응답 (패닉 아님)

### 1.2 Fault Injection 스텁
- `FailingRedis`: `RedisLikeStore` 구현, 모든 캐시 호출 실패 반환
- `FailingStore`: `DataStore` 구현, `save_board_post`/`save_blog_article`/`save_shop_order` 실패 반환

### 1.3 Docker 실 검증
- Redis 없는 환경 기동 → 요청 L1/L3 fallback으로 성공
- `{"code":"OK","id":1,"title":"Chaos Redis Down"}`

### 1.4 테스트
- 전체 104개 통과 (101 + 3 신규 chaos)

---

## 2. 주의사항

### 2.1 Graceful degradation 동작
- L2(Redis) 실패 → L3(DataStore) 직접 로드 (cache.rs `get_module` 로직)
- L3 실패 → `None` → 상위에서 CR0004 (모듈 미발견)
- save 실패 → `DataStoreError::DbQuery` → CR0001 (에러 응답)

### 2.2 FailingStore 한계
- 모듈 로딩(upsert/get)은 성공, save만 실패
- 실제 DB 완전 장애는 Docker에서 postgres stop으로 검증

### 2.3 Recovery
- invalidate는 L1만 제거 (L2 삭제 시도, 실패해도 무손상)
- 재요청 시 L3에서 재로드

---

## 3. 다음 단계 (Phase 31)

### 3.1 배포 자동화
- GitHub Actions CI/CD 파이프라인
- Docker image build + push (Registry)
- Health check 기반 rolling deploy
- Rollback 전략
- Environment 세팅 (dev/staging/prod)

### 3.2 Observability 강화
- Distributed tracing (OpenTelemetry)
- Structured logging enrichment
- SLO/SLI 정의
- Alert rules

---

## 4. 빌드 및 테스트

```bash
# 전체 테스트 (104개)
cargo test

# chaos test만
cargo test --test phase30_chaos

# Docker에서 Redis 장애 실 검증
docker compose up -d postgres meta_server  # redis 제외
curl -X POST http://localhost:3000/api/board \
  -H "Content-Type: application/json" \
  -d '{"title":"Chaos","body":"OK"}'
```
