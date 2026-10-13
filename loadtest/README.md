# Phase 29: 부하 테스트 가이드

## k6 설치

```bash
# Windows
winget install grafana.k6

# Linux (WSL)
curl -s https://dl.k6.io/ | grep k6 | head -1
# 또는 apt (Ubuntu)
sudo snap install k6
```

## 실행

```bash
# 기본 (기본 100→500 VUs, 65s total)
k6 run loadtest/k6.js

# 서버 주소 지정
META_SERVER_URL=http://localhost:3000 k6 run loadtest/k6.js

# Docker Compose 환경
META_SERVER_URL=http://127.0.0.1:3000 k6 run loadtest/k6.js
```

### Docker로 k6 실행 (로컬 k6 없이)
k6 바이너리가 없을 때 컨테이너로 실행. meta_server와 **동일 compose 네트워크**에 붙여 `meta_server:3000`으로 접근.

```bash
# SLO 벤치마크 (50 VU, 30s — 이 문서의 SLO 측정용)
# ※ Rate Limiter 배제 조건(RATE_LIMIT_PER_MIN 부하 이상)에서 실행해야 함. SLO.md 참고.
docker run --rm --entrypoint /bin/sh \
  --network meta_server_meta-net \
  -v $(pwd)/loadtest:/scripts:ro \
  -e META_SERVER_URL=http://meta_server:3000 \
  -e ADMIN_TOKEN=admin-secret-123 \
  loadimpact/k6 -c "k6 run /scripts/k6_slo.js"
```

> 공식 이미지: `loadimpact/k6` (grafana/k6). `alioz/k6`는 존재하지 않음.

## 시나리오

| Stage | VUs | Duration | 목적 |
|-------|-----|----------|------|
| warmup | 10 | 5s | health check warm-up |
| ramp | 100→500 | 30s | 부하 단계적 증가 |
| steady | 500 | 30s | 안정 부하 유지 |

## Thresholds (자동 실패 기준)

| Metric | Threshold | 의미 |
|--------|-----------|------|
| `http_req_duration` | p95 < 500ms | 95% 요청 500ms 이내 |
| `http_req_duration` | p99 < 1000ms | 99% 요청 1s 이내 |
| `http_req_failed` | rate < 5% | 에러율 5% 미만 |
| `checks` | rate > 95% | 체크 통과율 95% 이상 |

## 결과 해석

- **p95/p99 latency**: 95/99 백분위 응답 시간
- **throughput (req/s)**: 초당 처리 요청 수
- **error rate**: 실패한 요청 비율
- **VUs**: 동시 가상 사용자 수

## Bottleneck 분석

1. **CPU**: Grafana에서 CPU 사용률 확인 (Prometheus)
2. **Memory**: rquickjs memory limit 도달 여부
3. **DB**: PostgreSQL connection wait, query time
4. **Cache**: L1/L2 hit rate 확인 (`/api/admin/metrics`)

## Admin API 부하 테스트

```bash
ADMIN_TOKEN=admin-secret-123 k6 run loadtest/k6.js
```
