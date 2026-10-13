# HANDOVER: Phase 7 — L2 캐시 조회 활성화 + 실 트래픽 벤치마크

> 이 문서는 Phase 7을 완료하고 단절했을 때, 다음 개발자(또는 다음 세션의 AI 에이전트)가
> 이어서 작업을 진행할 수 있도록 작성된 핸드오버 문서입니다.

---

## 1. Phase 7 완료 내용

### 1.1 L1→L2→L3 3단계 캐시 조회

**설계 결정:** L1Cache에 `redis_store: Option<Arc<dyn RedisLikeStore>>` 추가 (O1-2 변형)

- L1(Memory) → L2(Redis/InMemoryPubSub) → L3(PostgreSQL/MemoryStore)
- `L1Cache.get_module()`에서 L1 Miss 시 L2에서 조회, Hit이면 L1 적재
- L2 Miss 시 L3에서 로드 → L2에 자동 적재 → L1에 적재
- `L1Cache.invalidate()`에서 L1 + L2(`module:{module_id}` 키) 동시 삭제

**흐름:**
```
get_module(module_id):
  1. L1.get → Hit → 반환
  2. L1 Miss → Single Flight 시작
     1. L2.get_cache("module:{module_id}")
        ├─ Hit → MetaModule 복원 → L1 적재 → 반환
        └─ Miss → L3.get_module(module_id)
             ├─ Hit → L2.set_cache(...) → L1 적재 → 반환
             └─ Miss → None (CR0004)
  3. Single Flight 종료
```

### 1.2 변경 파일

| 파일 | 변경 내용 |
|------|----------|
| `src/cache.rs` | `L1Cache`에 `redis_store` 필드 + `with_redis()` 빌더 + L2/L3 조회 로직 + `invalidate()`에 L2 삭제 |
| `src/meta/redis.rs` | `RedisLikeStore` 트레이트에 `delete_cache()` 추가 + `InMemoryPubSub`/`RealRedisStore` 구현 |
| `src/pipeline.rs` | `build_board_pipeline_with_store`에 `redis_store` 파라미터 추가 |
| `src/main.rs` | `redis_store`를 `build_board_pipeline_with_store`에 전달 |
| `tests/phase6_cache_flow.rs` | `redis_store` 파라미터 갱신 |
| `tests/phase6_benchmark.rs` | `redis_store` 파라미터 갱신 |
| `tests/phase7_benchmark.rs` | 신규 — 6개 테스트 (L2 Hit/Miss, 무효화, 동시 100개 WS, 적중률, L2 성능) |

### 1.3 벤치마크 결과

| 항목 | 실측 | 기준 | 상태 |
|------|------|------|------|
| L1 Miss → L2 Hit → L1 적재 | ✅ | 통과 | ✅ |
| L1 Miss → L2 Miss → L3 → L2/L1 적재 | ✅ | 통과 | ✅ |
| L2 무효화 시 L2에서 모듈 삭제 | ✅ | 통과 | ✅ |
| 동시 100개 WebSocket | 100/100 | 100/100 | ✅ |
| L1 적중률 | 99.0% (990/1000) | 90% 이상 | ✅ |
| L2 캐시 조회 성능 | 0.000ms/회 | 5ms 미만 | ✅ |

전체 테스트: **55개 통과** (Phase 1~7)

---

## 2. 주의사항

### 2.1 `L1Cache::with_redis()` 빌더 패턴

`L1Cache`는 `new(store)`로 생성하고, `with_redis(redis_store)`로 L2를 연결합니다.
L2가 없는 환경에서는 `with_redis()`를 호출하지 않으면 L3에 직접 조회합니다 (기존 동작 유지).

```rust
// L2 사용
let cache = Arc::new(L1Cache::new(store).with_redis(redis_store));

// L2 미사용 (L3 직접)
let cache = Arc::new(L1Cache::new(store));
```

### 2.2 `invalidate()`에서 L2 동시 삭제

`L1Cache.invalidate(module_id)`는 L1에서 모듈을 삭제하고,
L2에 연결되어 있으면 `module:{module_id}` 키를 L2에서도 삭제합니다.

```rust
cache.invalidate("base.board").await;
// L1: base.board 제거
// L2: module:base.board 제거
```

### 2.3 `delete_cache()` 트레이트 메서드

`RedisLikeStore` 트레이트에 `delete_cache(&self, key: &str)`가 추가되었습니다.
기존 구현체(`InMemoryPubSub`, `RealRedisStore`)에 구현이 추가되었습니다.
다른 `RedisLikeStore` 구현체가 있다면 `delete_cache`를 구현해야 합니다.

### 2.4 `build_board_pipeline_with_store` 시그니처 변경

```rust
// Phase 6 (이전)
pub async fn build_board_pipeline_with_store(
    engine: &Engine,
    store: Arc<dyn DataStore>,
    observability: Arc<Observability>,
) -> Result<BoardPipeline, DataStoreError>

// Phase 7 (현재)
pub async fn build_board_pipeline_with_store(
    engine: &Engine,
    store: Arc<dyn DataStore>,
    redis_store: Arc<dyn RedisLikeStore>,  // 신규
    observability: Arc<Observability>,
) -> Result<BoardPipeline, DataStoreError>
```

기존 테스트에서 `InMemoryPubSub::new()`로 대체:
```rust
let redis_store: Arc<dyn RedisLikeStore> = Arc::new(InMemoryPubSub::new());
```

### 2.5 L2 캐시 키 형식

`module:{module_id}` — 예: `module:base.board`, `module:ext.notice`

`CacheEntry.value`는 `MetaModule`의 JSON 직렬화.

---

## 3. 다음 단계 (Phase 8)

### 3.1 실 Redis/PostgreSQL 연동
- Docker 또는 로컬 인스턴스로 실 Redis + PostgreSQL 기동
- `REDIS_ENABLED=true` + `REDIS_URL=redis://localhost:6379`
- `DATABASE_BACKEND=postgres` + `DATABASE_URL=postgres://user:pass@localhost:5432/meta`
- `migrations/001_init.sql` 실행
- 실 환경에서 L1→L2(Redis)→L3(PG) 3단계 캐시 실증

### 3.2 다중 인스턴스 L1 무효화
- 2개 이상 서버 인스턴스 기동
- 인스턴스 A에서 `publish_invalidation` → 인스턴스 B의 L1 캐시 무효화 실증
- 실 Redis Pub/Sub으로 검증

### 3.3 운영 가이드 문서
- 환경변수 정리 (DATABASE_BACKEND, DATABASE_URL, REDIS_ENABLED, REDIS_URL, SOCKET_MAX_SESSIONS)
- Docker Compose 예제
- 성능 튜닝 가이드 (SOCKET_MAX_SESSIONS, CPU_TIMEOUT_MS, MEMORY_LIMIT)
- API 문서 (REST `/api/board` + WebSocket `/ws` protocol)

### 3.4 성능 벤치마크 확장
- 실 Redis 환경에서 L2 캐시 조회 지연 측정
- 동시 500개 WebSocket 연결
- 실 트래픽 패턴 (무작위 module_id, 실 버전 변경)

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
