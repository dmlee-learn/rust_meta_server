# Phase 26 설계: 성능 최적화

> Connection Pooling 모니터링, Cache Warming, Query optimization.

---

## 1. Cache Warming (가장 큰 효과)

### D26-1: `L1Cache::warmup(module_ids)`
- 기동 시 모든 base/extension 모듈을 L1에 사전 적재
- 대상 모듈:
  - `base.board`, `ext.notice`
  - `base.erp`, `ext.erp_po`
  - `base.blog`, `ext.blog_tech`
  - `base.shop`, `ext.shop_electronics`
- 기존 `get_module()` 재사용 (L2→L3 로드 로직 포함, Single Flight 안전)
- 실패해도 기동 중단 안 함 (무선속성: 첫 요청 시 다시 로드)

### D26-2: 기동 시점
- `main.rs`에서 파이프라인 생성 후, router 빌드 전에 warmup 호출
- 매 파이프라인의 `cache`에 warmup (또는 공용 모듈 목록 한 번만)

---

## 2. Connection Pooling 모니터링

### D26-3: PostgreSQL
- 현재 `tokio_postgres::Client` 단일 연결 사용 (sqlx pool 아님)
- pg pool이 없으므로 pool stats 대신 다음 공개:
  - `database.backend`
  - `database.migrations_applied`
  - `database.warmup_modules` (warmup된 모듈 수)
- pg pool 추가는 큰 리팩토링이라 Phase 26 범위 밖 (기존 retry 로직 유지)

### D26-4: Redis
- `MultiplexedConnection` (multiplexed 단일 연결, 내부에서 다중화)
- Redis 상태: `redis.enabled`
- `INFO` 기반으로 connected_clients 공개 (선택)

---

## 3. Admin Status에 성능 정보 추가

### D26-5: `GET /api/admin/status` 확장
- `database.warmup_modules`: 기동 시 적재된 모듈 수
- `database.backend`: postgres/memory

### D26-6: `GET /api/admin/ping` 확장 (선택)
- DB 응답 시간 측정 (SELECT 1)
- Redis 응답 시간 측정 (PING)
- 응답에 `db_latency_ms`, `redis_latency_ms` 포함

---

## 4. 에러 코드

Phase 26은 신규 에러 코드 없음 (기존 재사용).

---

## 5. 파일 변경

| 파일 | 변경 |
|------|------|
| `src/cache.rs` | `warmup(module_ids: &[&str])` 메서드 추가 |
| `src/main.rs` | 기동 시 cache warming 호출 |
| `src/http.rs` | `admin_status`에 warmup 정보, `admin_ping`에 DB/Redis latency |

---

## 6. 검증

- 기동 로그에 `[L1] warmup: N modules` 출력
- 첫 요청이 warm 상태인지 확인 (L1 히트)
- `/api/admin/status`에서 `warmup_modules` 확인
- 전체 테스트 통과
