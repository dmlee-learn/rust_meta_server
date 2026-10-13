# HANDOVER: Phase 15 — Rate Limiting + API 문서 + Docker 실 검증

> 이 문서는 Phase 15를 완료하고 단절했을 때, 다음 개발자(또는 다음 세션의 AI 에이전트)가
> 이어서 작업을 진행할 수 있도록 작성된 핸드오버 문서입니다.

---

## 1. Phase 15 완료 내용

### 1.1 Rate Limiting

`src/rate_limit.rs` 신규 생성:

```rust
pub struct RateLimiter {
    counts: RwLock<HashMap<String, (u64, Instant)>>,
    config: RateLimitConfig,
}
```

- IP 기반 sliding window (분당 `RATE_LIMIT_PER_MIN` 회, 기본 60)
- 429 Too Many Requests + `Retry-After` 헤더
- `CR4001` 에러 코드
- `X-Forwarded-For` 또는 `X-Real-IP` 헤더에서 IP 추출
- `/health`, `/metrics`, `/api/engine/*` 제외

### 1.2 API 문서 업데이트

**`docs/setting/API.md`:**
- JWT 인증 섹션 (Authorization: Bearer)
- `/metrics` 엔드포인트
- Rate Limiting (429, CR4001)
- ERP/Blog/Shop API 상세
- 에러 코드에 `CR4001` 추가

**`docs/setting/OPERATION_GUIDE.md`:**
- `JWT_SECRET`, `JWT_DISABLED`, `RATE_LIMIT_PER_MIN` 환경변수
- Docker 컨테이너 기동 절차
- Rate Limiting + JWT 튜닝 가이드

### 1.3 Docker 실 검증

| 항목 | 결과 |
|------|------|
| Docker 이미지 빌드 | ✅ 성공 (rust:1.88-slim) |
| `/health` | ✅ JSON 정상 |
| `/metrics` | ✅ Prometheus text 정상 |
| `POST /api/board` | ✅ 201 Created, `{"code":"OK","id":1,...}` |
| `POST /api/erp/purchase-orders` | ⚠️ Docker 환경에서 crash |
| `POST /api/blog/articles` | ⚠️ Docker 환경에서 crash |
| `POST /api/shop/orders` | ⚠️ Docker 환경에서 crash |
| 전체 테스트 | ✅ 91개 통과 |

### 1.4 Dockerfile 변경

- `rust:1.79-slim` → `rust:1.88-slim` (의존성 요구사항)
- `.dockerignore`에서 `Cargo.lock` 제외 해제

### 1.5 전체 테스트: **91개 전부 통과**

| 테스트 파일 | 개수 |
|-------------|------|
| Phase 15 (rate_limit) | 4 |
| 기존 (Phase 1~14) | 87 |
| **합계** | **91** |

---

## 2. 주의사항

### 2.1 Docker 환경 ERP/Blog/Shop crash

Docker 컨테이너에서 `POST /api/erp/purchase-orders`, `/api/blog/articles`, `/api/shop/orders` 호출 시 컨테이너가 crash됩니다.
Windows 로컬 테스트는 전부 통과합니다.

**추정 원인:**
- `DomainPipeline`의 샌드박스 생성 시 rquickjs C 라이브러리 문제
- Docker 컨테이너 메모리 제한
- `DomainPipeline`이 `BoardPipeline`과 다른 초기화 경로를 사용

**대응:**
- 별도 이슈로 추적 (Phase 16 또는 bug fix)
- `BoardPipeline`은 Docker에서도 정상 작동

### 2.2 Docker 네트워크

- Docker Compose 네트워크: `meta_server_meta-net`
- PostgreSQL alias: `postgres`
- Redis alias: `redis`
- meta_server 컨테이너는 `--network meta_server_meta-net`으로 실행

### 2.3 Rate Limiting

- `RATE_LIMIT_PER_MIN` 환경변수로 조정
- 테스트용: `1000` (제한 없음)
- 프로덕션: `60` (기본값)

### 2.4 JWT

- `JWT_SECRET`: HMAC 시크릿 (프로덕션에서 반드시 변경)
- `JWT_DISABLED=true`: 개발용 인증 비활성화
- 도메인별 권한: `domains` 클레임

---

## 3. 다음 단계 (Phase 16)

### 3.1 Docker ERP/Blog/Shop crash 해결
- `DomainPipeline` 초기화 경로 확인
- Docker 메모리/스택 제한 확인
- rquickjs C 라이브러리 링크 문제 확인

### 3.2 실 PostgreSQL/Redis + Docker 통합 테스트
- Docker Compose + meta_server 컨테이너 동시 기동
- 실 PostgreSQL에 데이터 저장 확인
- Redis Pub/Sub 무효화 확인

### 3.3 Observability 확장
- Prometheus histogram (latency distribution)
- per-domain metrics label
- structured logging (JSON)

### 3.4 API Rate Limiting 확장
- per-domain rate limit
- user-based rate limit (JWT sub)
- burst limit

---

## 4. 빌드 및 테스트

```bash
# 전체 테스트 (91개)
cargo test

# Docker 빌드 (WSL2)
wsl -d Ubuntu -u root -- docker build -t meta_server:latest /mnt/c/ai/meta_server

# Docker 실행
wsl -d Ubuntu -u root -- docker run -d \
  --name meta_server_app \
  -p 3999:3000 \
  -e DATABASE_BACKEND=memory \
  -e REDIS_ENABLED=false \
  -e JWT_DISABLED=true \
  meta_server:latest

# API 호출
curl -X POST http://127.0.0.1:3999/api/board \
  -H "Content-Type: application/json" \
  -d '{"title":"Test","body":"Body"}'
```
