# HANDOVER: Phase 21 — WebSocket Domain + Monitoring + OpenAPI

> 이 문서는 Phase 21을 완료하고 단절했을 때, 다음 개발자(또는 다음 세션의 AI 에이전트)가
> 이어서 작업을 진행할 수 있도록 작성된 핸드오버 문서입니다.

---

## 1. Phase 21 완료 내용

### 1.1 WebSocket Domain Topic
- `ws_handler()`에 `Query(params)` 추가
- `?domain=erp` 쿼리 파라미터로 domain 지정
- `handle_socket()`에 `domain` 파라미터 전달
- welcome message에 `domain` 필드 포함

### 1.2 Grafana + Prometheus
- `grafana/provisioning/datasources/prometheus.yml` — Prometheus datasource
- `grafana/provisioning/dashboards/dashboards.yml` — Dashboard provider
- `grafana/provisioning/dashboards/meta_server.json` — Meta Server dashboard
- `prometheus.yml` — Prometheus scraping config
- Docker Compose: Grafana (3001) + Prometheus (9090) 서비스 추가

### 1.3 OpenAPI 3.0 Spec
- `docs/openapi.yaml` — OpenAPI 3.0 spec
- 엔드포인트: `/health`, `/metrics`, `/api/board`, `/api/erp`, `/api/blog`, `/api/shop`, `/ws`

### 1.4 Docker Compose 실 검증
- 5개 컨테이너 기동: redis, postgres, meta_server, prometheus, grafana
- meta_server: healthy
- `/health`: `status: ok`
- `POST /api/board`: `{"code":"OK","id":1,"title":"Phase 21 Test"}`
- Grafana: `{"database": "ok", "version": "13.2.3"}`

### 1.5 테스트
- 전체 96개 테스트 통과

---

## 2. 주의사항

### 2.1 WebSocket Domain
WebSocket 연결 시 `?domain=erp`를 사용해야 domain별 필터링이 적용됩니다.
domain이 없으면 전체 이벤트를 수신합니다.

### 2.2 Grafana 접근
- URL: `http://localhost:3001`
- 계정: `admin` / `admin`
- Dashboard: "Meta Server Dashboard"

### 2.3 Prometheus 접근
- URL: `http://localhost:9090`
- Scraping: `meta_server:3000/metrics` (15s interval)

### 2.4 OpenAPI Spec
- `docs/openapi.yaml` — Swagger UI에서 열람 가능
- 엔드포인트별 요청/응답 스키마 포함

---

## 3. 다음 단계 (Phase 22)

### 3.1 Performance Optimization
- Connection pooling
- Query optimization
- Cache warming

### 3.2 Security Hardening
- CORS 정책
- CSRF protection
- Input validation 강화

### 3.3 Testing
- Integration test suite
- Load testing
- Chaos engineering

### 3.4 Documentation
- Architecture diagram
- API usage examples
- Deployment guide

---

## 4. 빌드 및 테스트

```bash
# 전체 테스트 (96개)
cargo test

# Docker Compose 기동 (5개 컨테이너)
cd /mnt/c/ai/meta_server
docker compose up -d

# API 호출
curl -X POST http://localhost:3000/api/board \
  -H "Content-Type: application/json" \
  -d '{"title":"Test","body":"Body"}'

# WebSocket (domain 지정)
# wscat -c ws://localhost:3000/ws?domain=erp

# Grafana
# http://localhost:3001 (admin/admin)

# Prometheus
# http://localhost:9090

# Docker Compose 종료
docker compose down
```
