# HANDOVER: Phase 6 — 다층 캐시 연동 + 성능 벤치마크

> 이 문서는 Phase 6을 완료하고 단절했을 때, 다음 개발자(또는 다음 세션의 AI 에이전트)가
> 이어서 작업을 진행할 수 있도록 작성된 핸드오버 문서입니다.

---

## 1. Phase 6 완료 내용

### 1.1 다층 캐시 흐름 실증

**설계 결정 (O1-2):** L2를 Pub/Sub만 유지

- L1(Memory) → L3(MemoryStore) 직결 (현재 구조 유지)
- L2(InMemoryPubSub)는 **Pub/Sub 무효화**만 수행
- L2 캐시 조회 기능은 실 Redis 연동(Phase 7)에서 활성화

**실증 시나리오:**
```
1. POST /api/board → L1 적재 (base.board v1, ext.notice v1)
2. L1Cache.invalidate("base.board") → L1에서 base.board 제거
3. L3(MemoryStore)에 base.board v2 upsert
4. L1Cache.get_module("base.board") → L1 Miss → L3 v2 조회 → L1 재적재 (version=2)
```

### 1.2 성능 벤치마크 결과

| 항목 | 실측 | 기준 | 상태 |
|------|------|------|------|
| L1 캐시 적중률 | 100% (99/99) | 90% 이상 | ✅ |
| 훅 발동 성능 | 1.00ms/회 (50회) | 10ms 미만 | ✅ |
| 무한 루프 감지 | 500.08ms (cpu_timeout_ms=500) | 600ms 미만 | ✅ |
| 동시 WebSocket | 50/50 성공 | 50/50 | ✅ |
| 세션 풀 한계 | 5 성공 + 1 CR2001 (max=5) | 5 + 1 | ✅ |

### 1.3 테스트 파일

| 파일 | 테스트 수 | 내용 |
|------|----------|------|
| `tests/phase6_cache_flow.rs` | 2개 | L1 무효화 → 재적재 시나리오 |
| `tests/phase6_benchmark.rs` | 5개 | 성능 벤치마크 (적중률, 훅 성능, 무한 루프, 동시 WS, 세션 한계) |

전체 테스트: **55개 통과** (Phase 1: 17, Phase 2: 10, Phase 3: 6, Phase 4: 11, Phase 5: 3, Phase 6: 7, +1 lib test)

---

## 2. 주의사항

### 2.1 L1 무효화 후 `get_module` 주의

`L1Cache.invalidate(module_id)` 후 `get_module(module_id)`를 호출하면,
**Single Flight**로 L3에서 모듈을 다시 로드하고 L1에 적재합니다.
따라서 **무효화 확인은 `len()`으로** 해야 합니다. `get_module`은 L3에 모듈이 있으면
무효화 후에도 `Some`을 반환합니다 (L1에 다시 적재되기 때문).

```rust
// ❌ 오해: 무효화 후 get_module이 None을 기대
cache.invalidate("base.board").await;
assert!(cache.get_module("base.board").await.is_none());  // 실패 — L3에서 다시 적재

// ✅ 올바름: len()으로 무효화 확인
cache.invalidate("base.board").await;
assert_eq!(cache.len().await, 1);  // base.board만 제거, ext.notice만 남음
```

### 2.2 `InfraError` 디버그 형식

`InfraError::Cj102`의 디버그 형식은 `Cj102`입니다.
`CJ1102`가 아니라 `Cj102`로 매칭해야 합니다.

```rust
let err_str = format!("{:?}", err);
assert!(err_str.contains("Cj102") || err_str.contains("CJ1102"));
```

### 2.3 `Observability::new` 시그니처

`Observability::new(observer: Arc<dyn ScriptObserver>, slow_threshold_ms: u64)`

```rust
let observability = Arc::new(Observability::new(Arc::new(NoopObserver), 500));
```

### 2.4 `DataStore` 트레이트 + `Arc`

`upsert_module`은 `DataStore` 트레이트의 메서드.
`Arc<MemoryStore>`에서 직접 호출하려면 `Arc<dyn DataStore>`로 타입 캐스팅해야 합니다.

```rust
let store: Arc<dyn DataStore> = Arc::new(MemoryStore::new());
store.upsert_module(&module).await.unwrap();
```

---

## 3. 다음 단계 (Phase 7)

### 3.1 실 Redis 연동
- `REDIS_ENABLED=true` + `REDIS_URL=redis://localhost:6379`
- `RealRedisStore`로 실제 Redis 인스턴스 연동
- L2 캐시 조회 기능 활성화 (L1 Miss → L2(Redis) 조회 → L3(PG) 조회)
- Pub/Sub으로 다중 인스턴스 L1 무효화 실증

### 3.2 실 PostgreSQL 연동
- `DATABASE_URL=postgres://user:pass@localhost:5432/meta`
- `migrations/001_init.sql` 실행
- `PostgresStore`로 실제 PG 인스턴스 연동
- L3(PG)에서 모듈 조회 + 게시판 저장 실증

### 3.3 L2 캐시 조회 활성화
- `BoardPipeline`에 L2(Redis) 조회 단계 추가
- L1 Miss → L2(Redis) 조회 → Hit이면 L1 적재, Miss이면 L3(PG) 조회 → L2/L1 적재
- `InMemoryPubSub`에도 캐시 조회를 동일하게 구현 (일관성 보장)

### 3.4 성능 벤치마크 확장
- 실 Redis/Pub/Sub 환경에서 L1 무효화 → L2 조회 → L3 조회 지연 측정
- 동시 100개 WebSocket 연결 + 메시지 송수신 지연
- L1 캐시 적중률 (실 트래픽 패턴)

---

## 4. 빌드 및 테스트

```bash
source /c/temp/load_msenv.sh
export RUSTFLAGS='-C link-arg=/LIBPATH:C:\wsdk10\10.0.26100.0\um\x64 -C link-arg=/LIBPATH:C:\wsdk10\10.0.26100.0\ucrt\x64'
cargo test
```

전체 55개 테스트 통과 확인:
```
test result: ok. 55 passed; 0 failed
```
