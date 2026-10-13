# SLO (Service Level Objective) — 메타서버

> 이 문서는 메타서버의 서비스 수준 목표를 **동결(결정)** 한 기준 문서입니다.
> Phase 40 SLO 벤치마크(k6)에서 측정된 실측 데이터를 근거로 작성.
> 측정 일시: 2026-10-06, k6 `loadimpact/k6:latest`, 50 VU × 30초.

## 1. 측정 환경
| 항목 | 값 |
|---|---|
| 빌드 | Rust release (Docker Linux) |
| 백엔드 | PostgreSQL (L3) + Redis (L2) + L1 인메모리 |
| 부하 도구 | k6 (`loadtest/k6_slo.js`) |
| 부하 | 50 VU, 30초, 단일 컨테이너(동일 IP) |
| 컨디션 | Rate Limiter 배제 (`RATE_LIMIT_PER_MIN=60000`으로 일시 상승 후 측정, 측정 후 100으로 원복) |
| 요청 구성 | `/health`·`/api/board`(POST)·`/api/tenant/posts`(GET)·`/api/admin/modules`(GET, admin token) 1:1:1:1 |

## 2. SLO 목표 (동결)
| 지표 | 목표 | 측정값 | 판정 |
|---|---|---|---|
| **p95 라텍시** | < 500ms | **79.6ms** | ✅ 충족 |
| **p99 라텍시** | < 1000ms | p95=79.6ms 기준 여유 (p99 미표기, max=174ms) | ✅ 충족 |
| **평균 라텍시** | < 100ms | 31.8ms | ✅ 충족 |
| **에러률** (4xx/5xx, 429 제외) | < 5% | **0.00%** | ✅ 충족 |
| **처리량** | > 500 req/s @ 50 VU | 606 req/s | ✅ 충족 |
| **가용성** (health 200) | ≥ 99.9% | 100% (벤치마크 구간) | ✅ 충족 |

## 3. k6 threshold (자동 검증 기준)
`loadtest/k6_slo.js`와 `loadtest/k6.js`의 `thresholds`:
```js
http_req_duration: ['p(95)<500', 'p(99)<1000'],
http_req_failed:   ['rate<0.05'],
```
이 threshold를 통과하면 SLO를 위반하지 않은 것으로 판정. CI/벤치마크 파이프라인에서 이 threshold가 실패하면 회귀(regression)로 간주.

## 4. ⚠️ Rate Limiter와 SLO의 관계
- **Rate Limit(429, 에러코드 CR4001)은 "실패"가 아니다.** 이는 Phase 18/39에서 구현한 보호 계층이며, `RATE_LIMIT_PER_MIN`(기본 100) 초과 시 정상 동작.
- SLO의 "에러률"은 **429를 제외**하고 측정한다. 429는 서버가 의도적으로 초과 부하를 거부하는 신호.
- `http_req_failed` threshold는 429를 포함하므로, **Rate Limiter가 활성 상태일 때 부하 테스트를 돌리면 threshold가 무조건 실패**한다. 이는 서버 결함이 아니라 Rate Limiter의 정상 동작.
- **SLO 벤치마크 시에는 Rate Limiter를 배제**(`RATE_LIMIT_PER_MIN`을 부하 수준 이상으로 일시 상승)하고 측정해야 한다. 이 문서의 측정값은 그 조건.

### Rate Limiter 동작 검증 (별도)
| 부하 | Rate Limit(100/min) | 결과 | 판정 |
|---|---|---|---|
| 50 VU ~980 req/s | 활성(100/min) | ~50% 요청이 `429 CR4001` | ✅ Rate Limiter 정상 동작 (과부하 거부) |
| 50 VU ~606 req/s | 배제(60000/min) | 0% 429, 100% 성공 | ✅ 서버 순수 성능 정상 |

두 결과 모두 **정상 동작**의 증거:
1. 과부하 시 Rate Limiter가 정확히 429로 거부(서버 보호).
2. Rate Limit을 배제하면 서버가 SLO를 크게 충족하는 성능을 제공.

## 5. 회귀 기준 (동결)
- 새로운 릴리스마다 `k6 run loadtest/k6_slo.js`를 Rate Limiter 배제 조건에서 실행.
- 아래 중 하나라도 충족되지 않으면 SLO 위반(회귀):
  - p95 ≥ 500ms
  - 에러률(429 제외) ≥ 5%
  - 처리량 < 500 req/s @ 50 VU
- 429 발생은 Rate Limiter가 의도적으로 작동한 것이므로 회귀가 아님 (Rate Limit 설정값 확인).

## 6. 운영 권장
- **Rate Limiter 기본값**: `RATE_LIMIT_PER_MIN=100`. 트래픽이 늘면 이 값을 환경변수로 조정.
- **SLO 모니터링**: Prometheus `/metrics`의 `http_request_duration` 히스토그램으로 실서비스 p95/p99 추적. k6 벤치마크는 정합성(regression gate) 용도.
- **Circuit Breaker**: 5xx 연속(문턱 `CIRCUIT_FAILURE_THRESHOLD`, 기본 10) 시 `Open` → 429 `CR4004`로 백엔드 보호. `/health`에서 상태 확인.

## 7. 다음 개발자 체크리스트
- [x] SLO 목표가 수치로 동결되어 있는가? → **Yes** (상표 2).
- [x] Rate Limit과 SLO 에러를 분리했는가? → **Yes** (429를 에러로 세지 않음).
- [x] 자동 검증 기준(k6 threshold)이 정의되었는가? → **Yes** (상표 3).
- [x] 실측 근거가 있는가? → **Yes** (상표 1 + 4, 2026-10-06 k6 실측).
