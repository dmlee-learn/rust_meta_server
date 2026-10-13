# Phase 35 설계: Observability 고급

> Distributed tracing (lightweight span) + SLO error budget metric + dashboard.

---

## 1. Lightweight Request Tracing

### D35-1: request-scope span
- `request_id_middleware`에서 request_id 기반으로 스텝별 duration 기록
- 각 도메인 처리 단계(board/erp/blog/shop)별耗时 추적
- `/api/admin/metrics`에 단계별 평균 시간 추가

### D35-2: Step timing
- pipeline handle_create/handle_request wrapper에서 단계별 측정
- before_create / after_save / save 구분

---

## 2. SLO Error Budget Metric

### D35-3: error_budget metric
- `Metrics`에 `error_budget_remaining` gauge
- 24h window 내 허용 에러 수 - 실제 에러 수
- SLO 99.9% → 24h 허용 에러 = total_requests * 0.001

### D35-4: Prometheus exposition
- `slo_availability_ratio` (24h)
- `error_budget_remaining`

---

## 3. Dashboard

### D35-5: SLO dashboard 확장
- error budget panel
- step-level timing breakdown

---

## 4. 파일 변경

| 파일 | 변경 |
|------|------|
| `src/metrics.rs` | SLO ratio + error budget metric |
| `src/http.rs` | request 단계 timing |
| `monitoring/grafana-dashboards/slo.json` | error budget panel |

---

## 5. 검증

- `/metrics`에 SLO metric 확인
- 전체 테스트 통과
