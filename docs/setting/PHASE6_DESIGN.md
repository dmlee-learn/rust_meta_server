# Phase 6 설계: 다층 캐시 연동 + 성능 벤치마크

> Phase 6에서 실 인스턴스 없이 메모리 스텁으로 L1↔L2↔L3 다층 캐시 흐름을 실증하고,
> 성능 벤치마크로 Phase 1~5의 안정성을 정량적으로 검증한다.

---

## 1. 설계 결정

### D1: L1 ↔ L2 ↔ L3 연동 방식

**결정: O1-2 — L2를 Pub/Sub만 유지**

- L1(Memory) → L3(MemoryStore) 직결 (현재 구조 유지)
- L2(InMemoryPubSub)는 **Pub/Sub 무효화**만 수행
- L2 캐시 조회 기능은 실 Redis 연동(Phase 7)에서 활성화

**이유:**
- AGENTS.md의 캐시 전략에서 L2는 "중앙 완충 지대"로, Pub/Sub 기반 무효화가 핵심 역할
- InMemoryPubSub 스텁에서 캐시 조회를 추가하면 실 Redis와 동작 차이 발생 가능
- Phase 6의 목표는 "다층 캐시 **흐름** 검증"이지 "L2 캐시 조회"가 아님

**실증 시나리오:**
```
1. POST /api/board → L1 적재 (base.board v1, ext.notice v1)
2. InMemoryPubSub.publish(InvalidationToken { module_id: "base.board", version: 2 })
3. L1Cache.invalidate("base.board") → L1 비워짐
4. POST /api/board → L1 Miss → L3(MemoryStore)에서 base.board v1 조회 → L1 재적재
   (v2가 L3에 없으므로 v1로 재적재 — L3에 v2가 upsert되어야 함)
```

**구현:**
- L1 무효화 시 `L1Cache.invalidate(module_id)` 호출
- L3에 `upsert_module`로 새 버전 등록 후, L1 재적재 시 새 버전 적재

### D2: 성능 벤치마크 항목

| 항목 | 측정 방법 | 기준 |
|------|----------|------|
| L1 캐시 적중률 | 동일 module_id 요청 100회, L1 Hit 수 / 총 요청 수 | 90% 이상 |
| 훅 발동 성능 | before_create 50회 발동, 평균 ms | 10ms 미만 |
| 무한 루프 감지 | CJ1102 발생까지 소요 시간 (cpu_timeout_ms=500ms) | 600ms 미만 |
| 동시 WebSocket | 50개 동시 연결 + welcome 수신 | 50/50 성공 |
| 세션 풀 한계 | SOCKET_MAX_SESSIONS=5, 6개 연결 | 5개 성공 + 1개 CR2001 |

### D3: 벤치마크 테스트 구조

**결정: O3-1 — `tests/phase6_benchmark.rs`**

- `#[tokio::test]`로 각 벤치마크를 개별 테스트
- `std::time::Instant`으로 측정
- `println!`로 기준 만족 여부 출력

### D4: L1 무효화 시나리오

**구현 파일:** `tests/phase6_cache_flow.rs`

**시나리오:**
1. `build_board_pipeline_with_store`로 파이프라인 생성 (L1 + L3)
2. `POST /api/board` → L1 적재 확인 (len=2)
3. `L1Cache.invalidate("base.board")` → L1 비워짐 확인 (len=1)
4. L3에 `upsert_module`으로 base.board v2 등록
5. `POST /api/board` → L1 Miss → L3 v2 조회 → L1 재적재 (version=2)
6. L1에 적재된 version=2 검증

---

## 2. 산출물

| 파일 | 내용 |
|------|------|
| `tests/phase6_cache_flow.rs` | L1 무효화 → 재적재 시나리오 (2개 테스트) |
| `tests/phase6_benchmark.rs` | 성능 벤치마크 (5개 테스트) |
| `docs/setting/PHASE6_DESIGN.md` | 이 문서 |
| `docs/setting/HANDOVER_phase_6_cache_benchmark.md` | Phase 6 핸드오버 |
| `docs/STATUS.md` | Phase 6 완료 상태 갱신 |

---

## 3. 테스트 세부 사항

### phase6_cache_flow.rs

```rust
// 테스트 1: l1_invalidation_and_reload
// 1. 파이프라인 생성
// 2. POST /api/board → L1 적재 (len=2)
// 3. L1Cache.invalidate("base.board")
// 4. len 확인 (len=1)
// 5. L3에 base.board v2 upsert
// 6. POST /api/board → L1 Miss → L3 v2 → L1 재적재
// 7. L1에서 base.board version=2 확인

// 테스트 2: l1_invalidation_multiple_modules
// 1. 파이프라인 생성
// 2. base.board + ext.notice 모두 invalidate
// 3. L3에 두 모듈 모두 v2 upsert
// 4. POST /api/board → 두 모듈 모두 L1 재적재 (v2)
```

### phase6_benchmark.rs

```rust
// 테스트 1: l1_cache_hit_rate
// - L1에 적재된 module_id 100회 조회
// - L1 Hit 수 측정 (기대: 100/100 = 100%)
// - 기준: 90% 이상

// 테스트 2: hook_execution_performance
// - before_create 훅 50회 발동
// - 각 회 평균 ms 측정 (std::time::Instant)
// - 기준: 10ms 미만

// 테스트 3: infinite_loop_detection_time
// - cpu_timeout_ms=500ms 엔진
// - 무한 루프 스크립트 실행
// - CJ1102 발생까지 시간 측정
// - 기준: 600ms 미만

// 테스트 4: concurrent_websocket_connections
// - 50개 동시 WebSocket 연결
// - 각 연결에서 welcome 수신 확인
// - 기준: 50/50 성공

// 테스트 5: session_pool_limit
// - SOCKET_MAX_SESSIONS=5
// - 6개 동시 연결
// - 5개 성공 + 1개 CR2001
```

---

## 4. 주의사항

1. **실 인스턴스 미사용**: Redis/PostgreSQL 인스턴스 없이 InMemoryPubSub + MemoryStore로만 테스트
2. **기존 테스트 회귀 금지**: Phase 1~5의 48개 테스트 전부 통과해야 함
3. **벤치마크 기준**: 기준 미달 시 `println!`로 경고 출력, 테스트는 통과 (기준은 참조용)
4. **동시성**: WebSocket 50개 동시 연결 시 port 충돌 방지 — 각 테스트마다 새 포트 사용
5. **에러 코드 6자리**: CJ1102, CR2001, CJ2101 등
