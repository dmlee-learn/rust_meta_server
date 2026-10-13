# Phase 7 설계: L2 캐시 조회 활성화 + 실 트래픽 벤치마크

> L1(Memory) → L2(Redis/InMemoryPubSub) → L3(PostgreSQL/MemoryStore) 3단계 캐시 조회를 구현하고,
> 실 트래픽 패턴 벤치마크로 다층 캐시 흐름의 성능을 검증한다.

---

## 1. 설계 결정

### D1: L2 캐시 조회 위치

**결정: O1-1 — `BoardPipeline`에 `RedisLikeStore` 추가**

- `BoardPipeline` 구조체에 `redis_store: Arc<dyn RedisLikeStore>` 필드 추가
- `handle_create`에서 L1 Miss 시 L2 조회 → Hit이면 L1 적재, Miss이면 L3 → L2/L1 적재
- `L1Cache`는 순수 인메모리 캐시로 유지 (Single Flight + invalidate만 담당)

**이유:**
- 책임 분리: L1Cache는 인메모리 캐시, BoardPipeline은 L2/L3 연동 관리
- L1Cache를 단순하게 유지하면 Single Flight 패턴의 복잡성이 증가하지 않음
- BoardPipeline이 L2/L3를 직접 관리하므로, 캐시 적재/무효화 로직이 한 곳에 집중

### D2: L2 캐시 키

`module:{module_id}` — 예: `module:base.board`, `module:ext.notice`

`CacheEntry`:
```json
{
  "key": "module:base.board",
  "value": "{\"module_id\":\"base.board\",\"kind\":\"base\",\"base_module_id\":null,\"script_source\":\"...\",\"content_hash\":\"...\",\"version\":1,\"enabled\":true}",
  "version": 1
}
```

`value`는 `MetaModule`의 JSON 직렬화.

### D3: L2 무효화

**흐름:**
1. `BoardPipeline`에서 L1 무효화 필요 시 `redis_store.publish_invalidation(token)` 호출
2. `InMemoryPubSub`/`RealRedisStore`의 `subscribe_invalidation` 수신자:
   - L1 캐시 무효화 (`L1Cache.invalidate(module_id)`)
   - L2 캐시에서 `module:{module_id}` 키 삭제

**구현:**
- `publish_invalidation` 호출 시, **발신자**가 L2에서 `module:{module_id}` 키를 직접 삭제
- 수신자는 L1 무효화만 수행 (기존 동작 유지)
- `InMemoryPubSub` + `RealRedisStore` 모두 동일 동작

### D4: L1 Miss → L2/L3 조회 흐름

```
handle_create:
  1. L1.get_module("base.board")
     ├─ Hit → L1에서 반환
     └─ Miss → L1에서 Single Flight 시작
         1. L2.get_cache("module:base.board")
            ├─ Hit → MetaModule 복원 → L1 적재 → 반환
            └─ Miss → L3.get_module("base.board")
                 ├─ Hit → MetaModule → L2.set_cache(...) → L1 적재 → 반환
                 └─ Miss → None (CR0004)
         2. Single Flight 종료
  2. L1.get_module("ext.notice") (동일)
  3. before_create 훅 발동
  4. after_save 훅 발동
```

### D5: 실 트래픽 벤치마크

| 항목 | 측정 방법 | 기준 |
|------|----------|------|
| 동시 100개 WebSocket | 100개 동시 연결 + welcome | 100/100 성공 |
| L1/L2/L3 적중률 | 1000회 요청, L1 Hit / L2 Hit / L3 Hit 비율 | L1 ≥ 90% |
| L2 캐시 조회 성능 | L1 Miss → L2 Hit 시 소요 시간 | 1ms 미만 (InMemoryPubSub) |
| L2 무효화 → 재적재 | invalidate 후 L2에서 모듈 삭제 확인 | L2에 모듈 없음 |

---

## 2. 구현 파일

| 파일 | 변경 내용 |
|------|----------|
| `src/pipeline.rs` | `BoardPipeline`에 `redis_store` 추가, L2/L3 조회 로직 |
| `src/meta/redis.rs` | `InMemoryPubSub`에 `delete_cache` 추가, `RealRedisStore`에 `delete_cache` 추가 |
| `src/meta/mod.rs` | `RedisLikeStore` 트레이트에 `delete_cache` 추가 |
| `src/main.rs` | `build_board_pipeline_with_store`에 `redis_store` 전달 |
| `tests/phase7_benchmark.rs` | 실 트래픽 벤치마크 (4개 테스트) |
| `tests/phase6_cache_flow.rs` | 기존 테스트 갱신 (L2 추가에 따른 시그니처 변경) |
| `tests/phase3_pipeline.rs` | 기존 테스트 갱신 (L2 추가에 따른 시그니처 변경) |
| `tests/functional_http.rs` | 기존 테스트 갱신 (L2 추가에 따른 시그니처 변경) |
| `tests/phase5_socket.rs` | 기존 테스트 갱신 (L2 추가에 따른 시그니처 변경) |

---

## 3. RedisLikeStore 트레이트 변경

기존:
```rust
pub trait RedisLikeStore: Send + Sync {
    async fn get_cache(&self, key: &str) -> Result<Option<CacheEntry>, RedisLikeError>;
    async fn set_cache(&self, entry: &CacheEntry) -> Result<(), RedisLikeError>;
    async fn publish_invalidation(&self, token: &InvalidationToken) -> Result<(), RedisLikeError>;
    async fn subscribe_invalidation(&self, handler: Box<dyn FnMut(InvalidationToken) + Send>) -> Result<(), RedisLikeError>;
}
```

변경:
```rust
pub trait RedisLikeStore: Send + Sync {
    async fn get_cache(&self, key: &str) -> Result<Option<CacheEntry>, RedisLikeError>;
    async fn set_cache(&self, entry: &CacheEntry) -> Result<(), RedisLikeError>;
    async fn delete_cache(&self, key: &str) -> Result<(), RedisLikeError>;  // 신규
    async fn publish_invalidation(&self, token: &InvalidationToken) -> Result<(), RedisLikeError>;
    async fn subscribe_invalidation(&self, handler: Box<dyn FnMut(InvalidationToken) + Send>) -> Result<(), RedisLikeError>;
}
```

`InMemoryPubSub` + `RealRedisStore` 모두 `delete_cache` 구현.

---

## 4. 주의사항

1. **기존 테스트 시그니처 변경**: `build_board_pipeline_with_store`에 `redis_store` 파라미터 추가 → 기존 테스트 전부 갱신
2. **L2 무효화 시점**: `publish_invalidation` 호출 시 **발신자**가 L2에서 키를 직접 삭제 (수신자는 L1 무효화만)
3. **InMemoryPubSub + RealRedisStore 동등 동작**: `delete_cache`가 두 구현체에서 동일하게 동작
4. **에러 코드 6자리**: CJ1102, CR2001, CJ2101 등
