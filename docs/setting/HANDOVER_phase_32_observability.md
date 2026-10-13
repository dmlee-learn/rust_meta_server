# HANDOVER: Phase 32 — Observability 강화

> 이 문서는 Phase 32를 완료하고 단절했을 때, 다음 개발자(또는 다음 세션의 AI 에이전트)가
> 이어서 작업을 진행할 수 있도록 작성된 핸드오버 문서입니다.

---

## 1. Phase 32 완료 내용

### 1.1 Request ID 미들웨어 (`src/http.rs`)
- `request_id_middleware`: 각 HTTP 요청에 `req-{timestamp}-{rand}` 부여
- 응답 헤더 `X-Request-Id` 반환
- structured logging: method, path, status, duration_ms, request_id (eprintln)
- router에서 가장 바깥 layer (CORS 뒤)

### 1.2 Prometheus Alert Rules (`monitoring/prometheus-alerts.yml`)
7개 SLO 기반 alert:
- HighErrorRate (5xx > 5%, critical)
- HighLatencyP95 (avg > 1000ms, warning)
- HighL1CacheMissRate (L1 miss > 50%, warning)
- HookErrorRateHigh (hook error > 0.5%, warning)
- SandboxCrashIncreasing (crash > 3/10m, critical)
- SandboxTimeoutIncreasing (timeout > 5/10m, warning)
- DBBackendDown (500 연속, critical)

### 1.3 SLO Dashboard (`monitoring/grafana-dashboards/slo.json`)
- Availability SLO (99.9%), Error Rate (<1%), Avg Latency (<500ms), Hook Success (>99.5%)
- Latency p95/p99, Throughput, Cache hit/miss, Sandbox errors timeseries

### 1.4 Docker Compose
- `prometheus.yml`: `rule_files: /etc/prometheus/rules/*.yml`
- `docker-compose.yml`: alert rules + SLO dashboard volume mount
- `grafana/provisioning/dashboards/dashboards.yml`: SLO provider 추가

### 1.5 Docker 실 검증
- `X-Request-Id: req-001a10161480c-1c74` 헤더 확인
- logging: `[http] request end: POST /api/board status=201 duration_ms=0.01 request_id=...`
- Prometheus: `slo_alerts` group, 7 alert rules 로드

### 1.6 테스트
- 전체 104개 통과

---

## 2. 주의사항

### 2.1 Docker build cache (중요)
- 이 환경에서 `docker compose build`가 stale image를 재사용 (무반응)
- 해결: `docker build -t meta_server:latest .` 로 직접 빌드 후 `docker compose up -d --force-recreate`
- buildkit cache 검증: `docker run --rm --entrypoint "" meta_server:latest grep -c "키워드" /usr/local/bin/meta_server`

### 2.2 request_id 생성
- 단순 timestamp + pseudo-random (UUID 라이브러리 없이)
- 높은 traffic에서 collision 가능성 극소 (timestamp + 16bit rand)
- 고유성 필요 시 uuid crate 추가

### 2.3 logging 방식
- `eprintln!` 사용 (tracing/log 라이브러리 미의존)
- 생산 환경에서는 structured logging (tracing)으로 교체 권장

### 2.4 Prometheus alerting
- Alertmanager 미연결 (alerting 비활성화)
- 실제 알림: Alertmanager + webhook/Slack 연동 필요

---

## 3. 다음 단계 (Phase 33)

### 3.1 성능 최적화 (고급)
- rquickjs context warm-up pool (pre-allocated)
- Connection pool tuning
- Query optimization
- Zero-copy serialization

### 3.2 프로덕션 강화
- Graceful shutdown (SIGTERM handling)
- Health check 강화 (deep check: DB/Redis 연결 확인)
- Config validation (startup 시)

---

## 4. 빌드 및 검증

```bash
# 전체 테스트 (104개)
cargo test

# Docker build (이 환경에서)
docker build -t meta_server:latest .
docker compose up -d --force-recreate meta_server

# X-Request-Id 확인
curl -s -D - -o /dev/null http://localhost:3000/health | grep x-request-id

# Prometheus alert 확인
curl -s http://localhost:9090/api/v1/rules
```
