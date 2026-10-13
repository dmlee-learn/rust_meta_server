# Phase 21 설계: WebSocket 확장 + Monitoring + API 문서

> WebSocket domain topic, Grafana/Prometheus, OpenAPI spec.

---

## 1. WebSocket 확장

### D21-1: Domain별 topic
- WebSocket 연결 시 `?domain=erp` 쿼리 파라미터로 domain 지정
- domain별 브로드캐스트: 해당 domain의 유효화 이벤트만 수신
- `/socket` 엔드포인트에 `domain` 쿼리 파라미터 추가

### D21-2: Real-time 유효화 알림
- Redis `meta:invalidation` 채널에서 수신 → WebSocket broadcast
- domain별 필터링

---

## 2. Monitoring Dashboard

### D21-3: Grafana provisioning
- `grafana/provisioning/datasources/prometheus.yml`
- `grafana/provisioning/dashboards/meta_server.json`
- Docker Compose에 Grafana 서비스 추가

### D21-4: Prometheus scraping
- `prometheus.yml` — meta_server `/metrics` 스크래핑
- Docker Compose에 Prometheus 서비스 추가

---

## 3. API Documentation

### D21-5: OpenAPI spec
- `docs/openapi.yaml` — OpenAPI 3.0 spec
- 엔드포인트: `/api/board`, `/api/erp`, `/api/blog`, `/api/shop`, `/health`, `/metrics`

### D21-6: Swagger UI
- `docs/setting/API.md` 업데이트
- OpenAPI spec 링크 추가

---

## 4. 산출물

| 파일 | 내용 |
|------|------|
| `src/socket.rs` | Domain topic + real-time 알림 |
| `grafana/provisioning/datasources/prometheus.yml` | Prometheus datasource |
| `grafana/provisioning/dashboards/meta_server.json` | Dashboard |
| `prometheus.yml` | Prometheus config |
| `docs/openapi.yaml` | OpenAPI 3.0 spec |
| `docker-compose.yml` | Grafana + Prometheus 서비스 |
