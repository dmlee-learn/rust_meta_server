# Phase 30 설계: Chaos Engineering

> DB/Redis 장애 시나리오 + graceful degradation + recovery 검증.

---

## 1. 시나리오

### D30-1: Redis 장애 (L2 Down) → L1/L3 fallback
- Redis가 응답 안 할 때 (InMemoryPubSub 실패 또는 timeout)
- L1 cache 히트 → 정상 응답
- L1 miss → L3(Store) 직접 로드
- graceful degradation: L2 실패해도 요청 성공

### D30-2: Cache 무효화 후 재로드 (Recovery)
- invalidate → L1 제거
- 재요청 → L3 로드 → L1 재적재
- recovery time 측정

### D30-3: DataStore 장애 시 에러 매핑
- Save 실패 → CR0001 / 적절한 에러 코드
- 요청이 패닉하지 않고 에러 응답 반환

### D30-4: L1 cache stampede 방어 재검증
- 동시 miss 시 Single Flight로 1회만 로드 (기존 test 재확인)

---

## 2. Test (`tests/phase30_chaos.rs`)

### T30-1: Redis 장애 fallback
- FailingRedisStore (모든 캐시 호출 실패 반환) 구현
- L1 미스 + L2 실패 → L3 로드 확인
- 요청 성공 검증

### T30-2: Recovery cycle
- invalidate → 재로드 → L1 적재 확인
- latency 재측정 (선택)

### T30-3: Save 장애 에러 매핑
- FailingStore (save 실패 반환) 구현
- handle_create → 에러 응답 확인 (패닉 아님)

---

## 3. 파일 변경

| 파일 | 변경 |
|------|------|
| `tests/phase30_chaos.rs` | chaos/fault-injection test |

---

## 4. 검증

- `cargo test --test phase30_chaos` 통과
- 기존 전체 테스트 통과
- graceful degradation 동작 확인
