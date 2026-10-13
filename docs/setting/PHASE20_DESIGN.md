# Phase 20 설계: rquickjs 시그널 + WebSocket 확장 + Monitoring

> rquickjs spawn_blocking eval, WebSocket domain topic, Grafana dashboard.

---

## 1. rquickjs C 라이브러리 시그널 문제 해결

### D20-1: spawn_blocking에서 JS eval
- `SandboxedContext::eval_string()`을 `tokio::task::spawn_blocking`에서 실행
- crash가 컨테이너 전체를 종료시키지 않도록 분리
- async 컨텍스트에서 sync blocking rquickjs 코드 실행

### D20-2: Docker 시그널 처리
- Docker `--restart=always` 유지

---

## 2. WebSocket 확장

### D20-3: Domain별 topic
- WebSocket 연결 시 `?domain=erp` 쿼리 파라미터로 domain 지정
- domain별 브로드캐스트: 해당 domain의 유효화 이벤트만 수신
- `/socket` 엔드포인트에 `domain` 쿼리 파라미터 추가

### D20-4: Real-time 유효화 알림
- Redis `meta:invalidation` 채널에서 수신 → WebSocket broadcast
- domain별 필터링

---

## 3. Monitoring Dashboard

### D20-5: Grafana provisioning
- `grafana/provisioning/datasources/prometheus.yml`
- `grafana/provisioning/dashboards/meta_server.json`
- Docker Compose에 Grafana 서비스 추가

### D20-6: Prometheus scraping
- `prometheus.yml` — meta_server `/metrics` 스크래핑
- Docker Compose에 Prometheus 서비스 추가

---

## 4. 산출물

| 파일 | 내용 |
|------|------|
| `src/engine/runtime.rs` | `spawn_blocking` JS eval |
| `src/socket.rs` | Domain topic + real-time 알림 |
| `grafana/provisioning/datasources/prometheus.yml` | Prometheus datasource |
| `grafana/provisioning/dashboards/meta_server.json` | Dashboard |
| `prometheus.yml` | Prometheus config |
| `docker-compose.yml` | Grafana + Prometheus 서비스 |
