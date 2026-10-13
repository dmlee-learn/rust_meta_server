# Phase 39 핸드오버: Rate Limiting 강화 (Token Bucket + Circuit Breaker)

## 1. 목표
기존 단순 카운터 기반 Rate Limiting을 **Token Bucket** 알고리즘으로 개선하고, 5xx 실패가 반복될 때 백엔드를 보호하는 **Circuit Breaker**를 추가한다.

## 2. 구현 내용
- `src/rate_limit.rs`:
  - `TokenBucket` — 최대 N개/분, 시간에 비례한 refill. 기존 단순 카운터보다 버스트 허용이 자연스러움.
  - `CircuitBreaker` — `Closed`/`Open`/`HalfOpen` 3상태. 일정 수(문턱)의 연속 실패 시 `Open`(재ject), 쿨다운 경과 후 `HalfOpen`(부분放行)으로 전환.
  - `CircuitState` enum.
- `src/http.rs`:
  - `AppState`에 `circuit_breaker: Arc<RwLock<CircuitBreaker>>` 필드 추가.
  - `rate_limit_middleware`에 회로 차단기 연결:
    - `Open` 상태면 요청을 **`CR4004`(429)**로 거부 (백엔드 보호).
    - 응답 status가 **5xx**면 `record_failure()`, 그 외(2xx/4xx)면 `record_success()`.
  - `/health` 엔드포인트에 `circuit_breaker` 상태(`state`, `failures`) 노출.
- `tests/phase39_circuit_breaker.rs`: 9개 테스트.
  - TokenBucket: refill, 제한, 버스트.
  - CircuitBreaker: 문턱 도달 시 Open, 쿨다운 후 HalfOpen, 성공 시 Closed 복귀.

## 3. 동작 요약
```
요청 → [CircuitBreaker Open?]
         ├─ Open  → 429 CR4004 (거부)
         └─ Closed/HalfOpen → [TokenBucket 허용?]
                ├─ 허용 안 됨 → 429 (rate limit)
                └─ 허용 → 백엔드 호출
                       ├─ 5xx 응답 → record_failure() (문턱 도달 시 Open 전환)
                       └─ 그 외   → record_success() (HalfOpen에서 Closed 복귀)
```
- **429(rate limit)는 Circuit Breaker 실패에 카운트되지 않는다** (옳은 동작 — 429는 서버 보호 신호이지 백엔드 실패가 아님).

## 4. 환경 변수
- `RATE_LIMIT_PER_MIN`: 분당 최대 요청 수 (기본 100).
- `CIRCUIT_FAILURE_THRESHOLD`: 연속 실패 문턱 (기본 10).
- `CIRCUIT_COOLDOWN_MS`: Open 상태에서 HalfOpen 전환까지 쿨다운 (기본 30000ms).

## 5. 검증 결과
- **로컬** `cargo test`: **117개 통과** (0 실패). Phase 39 테스트 9개 포함.

## 6. 에러 코드
- `CR4004`: Circuit Breaker Open 상태에서의 거부 (429).
- 기존 Rate Limit 429와 구분됨 (body에 `code: CR4004` 포함).

## 7. 다음 개발자 체크리스트
- [x] 백엔드 보호가 동작하는가? → **Yes** (5xx 연속 시 Open 전환, 거부).
- [x] 429가 회로 차단기 실패로 오기재되는가? → **No** (의도적으로 제외).
- [x] 에러 코드 명명 규칙 준수? → **Yes** (`CR4004`).
- [x] 상태 관찰 가능한가? → **Yes** (`/health` + Prometheus metrics).
