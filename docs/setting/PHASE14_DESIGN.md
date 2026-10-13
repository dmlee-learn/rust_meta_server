# Phase 14 설계: WSL2 Docker 실 검증 + Prometheus Metrics + JWT 인증

> Docker 이미지 실 기동, Prometheus metrics 엔드포인트, JWT 토큰 인증.

---

## 1. 설계 결정

### D1: Prometheus Metrics

**결정: `prometheus-client` crate로 `/metrics` 엔드포인트**

표현할 metrics:
| Metric | Type | 설명 |
|--------|------|------|
| `meta_server_http_requests_total` | Counter | 엔드포인트별 HTTP 요청 수 |
| `meta_server_http_request_duration_ms` | Histogram | 엔드포인트별 응답 시간 |
| `meta_server_cache_hits_total` | Counter | L1/L2/L3 캐시 적중 수 |
| `meta_server_cache_misses_total` | Counter | L1/L2/L3 캐시 미스 수 |
| `meta_server_hook_executions_total` | Counter | 훅 실행 수 |
| `meta_server_hook_errors_total` | Counter | 훅 에러 수 |
| `meta_server_active_sessions` | Gauge | 활성 WebSocket 세션 수 |
| `meta_server_engine_memory_bytes` | Gauge | rquickjs 엔진 메모리 사용량 |

### D2: JWT 인증

**결정: `jsonwebtoken` crate로 Bearer 토큰 인증**

- `Authorization: Bearer <token>` 헤더 체크
- 도메인별 권한: `{"sub": "user", "domains": ["erp", "blog", "shop"]}`
- 환경변수: `JWT_SECRET` (기본값: "dev-secret-change-me")
- `/health`, `/metrics`, `/api/engine/info`는 인증 불필요
- `/api/board`, `/api/erp/...`, `/api/blog/...`, `/api/shop/...`, `/ws`는 인증 필요

### D3: Docker 실 검증 (WSL2)

- `docker build` → 이미지 생성
- `docker compose up` → Redis + PostgreSQL 기동
- `docker run` meta_server → 실 PostgreSQL/Redis 연결
- `curl /health` → JSON 상태 확인
- API 요청 (board, erp, blog, shop) → 201 Created 확인

---

## 2. 산출물

| 파일 | 내용 |
|------|------|
| `src/metrics.rs` | Prometheus metrics 레이어 |
| `src/auth.rs` | JWT 인증 미들웨어 |
| `src/lib.rs` | `metrics`, `auth` 모듈 등록 |
| `src/http.rs` | `/metrics` 엔드포인트 + JWT 미들웨어 적용 |
| `Cargo.toml` | `prometheus-client`, `jsonwebtoken` 추가 |
| `tests/phase14_metrics_auth.rs` | Metrics + JWT 테스트 |

---

## 3. 주의사항

1. **prometheus-client API**: `Registry::new()`, `IntCounterVec::new()`, `TextEncoder::encode()`
2. **jsonwebtoken**: `EncodingKey::from_secret()`, `Header::default()`, `Validation::default()`
3. **Docker 네트워크**: meta_server 컨테이너에서 `postgres`/`redis` 서비스 이름으로 접근
4. **WSL2**: Docker 컨테이너 간 네트워크는 docker network로 자동 연결
