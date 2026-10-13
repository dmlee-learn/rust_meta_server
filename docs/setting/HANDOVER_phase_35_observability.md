# HANDOVER: Phase 35 — Observability 고급

> 이 문서는 Phase 35를 완료하고 단절했을 때, 다음 개발자(또는 다음 세션의 AI 에이전트)가
> 이어서 작업을 진행할 수 있도록 작성된 핸드오버 문서입니다.

---

## 1. Phase 35 완료 내용

### 1.1 SLO Metrics (`src/metrics.rs`)
- `Metrics.total_requests`: 총 HTTP 요청 카운터
- `Metrics.total_errors`: 총 5xx 에러 요청 카운터
- `encode()`에 SLO metric 추가:
  - `meta_server_slo_availability_ratio` (가용성 비율, 0~1)
  - `meta_server_slo_error_budget_ratio` (에러 버짓 소진 비율)
  - `meta_server_total_requests_total`
  - `meta_server_total_errors_total`

### 1.2 Request Middleware (`src/http.rs`)
- `request_id_middleware`가 `State<AppState>` 수신 (from_fn_with_state)
- 매 요청: `total_requests++`
- 5xx 응답 시: `total_errors++`

### 1.3 Grafana Dashboard (`monitoring/grafana-dashboards/slo.json`)
- `SLO Availability (24h, 99.9%)` stat panel
- `Error Budget Consumed` stat panel

### 1.4 Docker 실 검증
- `meta_server_slo_availability_ratio 1`
- `meta_server_slo_error_budget_ratio 0`
- `meta_server_total_requests_total 9`
- `meta_server_total_errors_total 0`
- 전체 104개 테스트 통과

---

## 2. 주의사항

### 2.1 SLO 계산 한계
- 가용성 비율은 **기동 후 누적** (restart 시 초기화)
- 24h sliding window 아님 — 단순 누적 기반
- true sliding window 필요 시 Prometheus `avg_over_time` 사용

### 2.2 Error Budget
- `slo_target = 0.999` 하드코딩
- `error_budget_ratio = slo_target - availability` (0.001 초과 시 소진)
- Grafana: `error_budget_ratio / (1 - 0.999)` → % 소진율

### 2.3 Middleware State
- `from_fn_with_state`로 AppState 전달
- `State` extact는 handler 첫 파라미터에 배치
- request_id_middleware는 `State<AppState>`가 첫 파라미터

---

## 3. 다음 단계 (Phase 36)

### 3.1 데이터 스키마 확장
- 게시판 카테고리/태그
- ERP 워크플로우 인스턴스
- Blog 댓글
- Shop 카트/결제

### 3.2 Multi-tenant
- 기업별 DB 격리
- tenant_id 기반 권한

---

## 4. 빌드 및 검증

```bash
# 전체 테스트 (104개)
cargo test

# Docker build
docker build -t meta_server:latest .
docker compose up -d --force-recreate meta_server

# SLO metric 확인
curl -s http://localhost:3000/metrics | grep -E "slo_|total_requests_total|total_errors_total"
```
