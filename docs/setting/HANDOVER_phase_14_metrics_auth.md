# HANDOVER: Phase 14 — Prometheus Metrics + JWT 인증

> 이 문서는 Phase 14를 완료하고 단절했을 때, 다음 개발자(또는 다음 세션의 AI 에이전트)가
> 이어서 작업을 진행할 수 있도록 작성된 핸드오버 문서입니다.

---

## 1. Phase 14 완료 내용

### 1.1 Prometheus Metrics

`src/metrics.rs` 신규 생성:

```rust
pub struct Metrics {
    pub http_requests: HttpRequests,      // 엔드포인트별 카운터
    pub cache_hits: CacheHits,           // L1/L2/L3 적중 카운터
    pub cache_misses: CacheMisses,       // L1/L2/L3 미스 카운터
    pub hook_executions: AtomicU64,      // 훅 실행 수
    pub hook_errors: AtomicU64,          // 훅 에러 수
    pub active_sessions: AtomicI64,      // 활성 WebSocket 세션
    pub engine_memory: AtomicI64,        // rquickjs 엔진 메모리
}
```

`/metrics` 엔드포인트:
- Prometheus text format (`# HELP`, `# TYPE`, metric lines)
- `content-type: text/plain; version=0.0.4`
- `/health`, `/metrics`는 인증 불필요

### 1.2 JWT 인증

`src/auth.rs` 신규 생성:

```rust
pub struct Claims {
    pub sub: String,
    pub domains: Option<Vec<String>>,  // None = 전부 허용
    pub exp: i64,
}
```

- `Authorization: Bearer <token>` 헤더 체크
- HS256 알고리즘
- 환경변수: `JWT_SECRET` (기본: "dev-secret-change-me"), `JWT_DISABLED` (기본: false)
- 도메인별 권한: `/api/erp/...` → `domains`에 "erp" 필요
- `/health`, `/metrics`, `/api/engine/*`은 인증 불필요

### 1.3 build_router 시그니처

```rust
pub fn build_router(
    engine: Engine,
    board: Option<Arc<BoardPipeline>>,
    domains: Vec<Arc<DomainPipeline>>,
    socket_config: SocketConfig,
    socket_script: String,
    db_backend: String,
    migrations: Vec<String>,
    redis_enabled: bool,
    jwt_secret: String,      // Phase 14
    jwt_disabled: bool,      // Phase 14
) -> Router
```

### 1.4 전체 테스트: **87개 전부 통과**

| 테스트 파일 | 개수 |
|-------------|------|
| Phase 14 (metrics_auth) | 4 |
| auth.rs 내장 테스트 | 3 |
| 기존 (Phase 1~13) | 80 |
| **합계** | **87** |

---

## 2. 주의사항

### 2.1 JWT 비활성화

개발/테스트 환경에서는 `JWT_DISABLED=true`로 JWT를 비활성화할 수 있습니다.
이 경우 모든 요청이 인증 없이 통과합니다.

### 2.2 Metrics 인스턴스

`Metrics`는 `AppState`에 `Arc<Metrics>`로 저장됩니다.
`AtomicU64`/`AtomicI64`은 `Clone`를 지원하지 않으므로 `Arc`를 통해 공유합니다.

### 2.3 prometheus-client 미사용

`prometheus-client` crate의 API가 복잡하여,
Prometheus text format을 직접 생성하는 lightweight `Metrics` 구현으로 대체했습니다.
동시성: `AtomicU64`/`AtomicI64`으로 lock-free.

### 2.4 도메인별 권한

JWT 토큰의 `domains` 필드가 `None`이면 모든 도메인 접근 허용.
`Some(["erp", "blog"])`이면 `/api/erp/...`와 `/api/blog/...`만 접근 가능.

---

## 3. 다음 단계 (Phase 15)

### 3.1 Docker 실 검증 (WSL2)
- `docker build` → 이미지 생성
- `docker run` + 실 PostgreSQL/Redis
- `curl /metrics` → Prometheus text 확인
- `curl /api/board` + JWT 토큰 → 201 Created

### 3.2 API 문서 업데이트
- `API.md`에 JWT 인증 + `/metrics` 추가
- `OPERATION_GUIDE.md`에 `JWT_SECRET`, `JWT_DISABLED` 환경변수 추가

### 3.3 Rate Limiting
- 도메인별 API 호출 제한
- IP 기반 rate limiting

---

## 4. 빌드 및 테스트

```bash
# 전체 테스트 (87개)
cargo test

# Phase 14 테스트만
cargo test --test phase14_metrics_auth

# JWT 생성 (개발용)
python3 -c "
import jwt, time
print(jwt.encode(
    {'sub': 'user1', 'domains': ['erp', 'blog', 'shop', 'board'], 'exp': int(time.time()) + 3600},
    'dev-secret-change-me',
    algorithm='HS256'
))"
```
