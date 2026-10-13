# Phase 32 설계: Observability 강화

> Structured logging enrichment + SLO/SLI 정의 + Prometheus alert rules.

---

## 1. Structured Logging enrichment

### D32-1: request_id 추적
- 각 HTTP 요청에 고유 request_id 부여 (UUID 또는 counter)
- logging, metrics, error logs에 request_id 포함
- 응답 헤더 `X-Request-Id` 반환

### D32-2: logging 확장
- `src/http.rs`: request 시작/종료 logging (method, path, status, duration, request_id)
- error 시 request_id 포함

---

## 2. SLO/SLI 정의

### D32-3: SLO (Service Level Objective)
| SLO | Target | SLI |
|-----|--------|-----|
| Availability | 99.9% | 200/1xx 응답 비율 |
| Latency p95 | < 500ms | http duration p95 |
| Error rate | < 1% | 4xx/5xx 비율 |
| Hook success | > 99.5% | hook_error / hook_execution |

### D32-4: SLO Dashboard (Grafana)
- `monitoring/grafana-dashboards/slo.json`
- SLO gauge panel + error budget

---

## 3. Prometheus Alert Rules

### D32-5: `monitoring/prometheus-alerts.yml`
- **HighErrorRate**: error rate > 5% (5분)
- **HighLatency**: p95 > 1s (5분)
- **CacheMissHigh**: L1 miss rate > 50%
- **HookErrors**: hook errors 증가
- **SandboxCrashes**: sandbox_crash_count 증가
- **SandboxTimeouts**: sandbox_timeout_count 증가
- **DBConnectionError**: DB backend down

---

## 4. 파일 변경

| 파일 | 변경 |
|------|------|
| `src/http.rs` | request_id middleware + logging |
| `src/metrics.rs` | SLO helper (선택) |
| `monitoring/prometheus-alerts.yml` | alert rules |
| `monitoring/grafana-dashboards/slo.json` | SLO dashboard |

---

## 5. 검증

- request_id 헤더 확인 (Docker 실 검증)
- logging output 확인
- 전체 테스트 통과
- Prometheus rule YAML 구문
