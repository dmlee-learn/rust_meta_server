# Phase 34 설계: 성능 최적화

> rquickjs engine warm-up + Redis PING (deep health 확장).

---

## 1. Engine Warm-Up

### D34-1: 기동 시 rquickjs warm-up
- 서버 기동 시 engine에 가벼운 스크립트 eval (JIT prewarm)
- `Engine::warm_up()`: `1+1` 또는 작은 루프 eval
- 첫 요청 latency 감소

### D34-2: 위치
- `Engine::warm_up(&self) -> Result<(), InfraError>`
- main.rs 기동 후 호출

---

## 2. Redis PING

### D34-3: `RedisLikeStore::ping()`
- trait에 `ping() -> Result<(), RedisLikeError>` 추가
- `InMemoryPubSub`: Ok
- `RealRedisStore`: PING 커맨드
- deep health check에서 사용

### D34-4: Deep health check 확장
- `/health/deep`: Redis enabled일 때 `ping()` 호출
- status: ok/degraded/down

---

## 3. 파일 변경

| 파일 | 변경 |
|------|------|
| `src/engine/runtime.rs` | `Engine::warm_up()` |
| `src/meta/redis.rs` | `RedisLikeStore::ping()` (trait + 구현체 2개) |
| `src/http.rs` | deep health check에 Redis PING |
| `src/main.rs` | engine warm_up 호출 |

---

## 4. 검증

- engine warm_up 테스트
- Redis PING Docker 실 검증 (enabled + disabled)
- 전체 테스트 통과
