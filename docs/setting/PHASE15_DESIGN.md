# Phase 15 설계: WSL2 Docker 실 검증 + Rate Limiting + API 문서

> Docker 컨테이너 실 기동, API 호출 검증, Rate Limiting 미들웨어, API 문서 업데이트.

---

## 1. 설계 결정

### D1: Rate Limiting

**결정: `tower`의 `Timeout` + `AtomicU64` 기반 simple sliding window**

- 도메인별 API 호출 제한 (기본: 분당 60회)
- 환경변수: `RATE_LIMIT_PER_MIN` (기본 60)
- 429 Too Many Requests 응답 + `Retry-After` 헤더
- IP 기반 (클라이언트 IP 추출)
- `RwLock<HashMap<IP, (count, window_start)>>`

### D2: WSL2 Docker 실 검증

**절차:**
1. WSL2 Ubuntu에서 Docker compose 기동 (Redis + PostgreSQL)
2. `docker build -t meta_server .` → 이미지 생성
3. `docker run` meta_server (PostgreSQL + Redis 연결)
4. `curl /health` → JSON 상태 확인
5. `curl /metrics` → Prometheus text 확인
6. `curl /api/board` (JWT 토큰) → 201 Created
7. `curl /api/erp/purchase-orders` → 201 Created
8. Rate Limiting 확인 (연속 요청 → 429)

### D3: API 문서 업데이트

**`docs/setting/API.md` 갱신:**
- JWT 인증 방법 (Authorization: Bearer)
- `/metrics` 엔드포인트
- Rate Limiting (429 응답)
- 에러 코드 추가 (CR4001: Rate Limit exceeded)

**`docs/setting/OPERATION_GUIDE.md` 갱신:**
- `JWT_SECRET`, `JWT_DISABLED` 환경변수
- `RATE_LIMIT_PER_MIN` 환경변수
- Docker 실 기동 절차

---

## 2. 산출물

| 파일 | 내용 |
|------|------|
| `src/rate_limit.rs` | Rate Limiting 미들웨어 |
| `src/lib.rs` | `rate_limit` 모듈 등록 |
| `src/http.rs` | Rate Limiting 미들웨어 적용 |
| `docs/setting/API.md` | JWT + metrics + rate limiting |
| `docs/setting/OPERATION_GUIDE.md` | 환경변수 + Docker 절차 |
| `tests/phase15_rate_limit.rs` | Rate Limiting 테스트 |

---

## 3. 주의사항

1. **Rate Limiting 키**: 클라이언트 IP (`X-Forwarded-For` 또는 `socket_addr`)
2. **429 응답**: `{"code": "CR4001", "message": "Rate limit exceeded"}` + `Retry-After`
3. **Docker 네트워크**: meta_server 컨테이너에서 `postgres`/`redis` 서비스 이름 사용
4. **WSL2 Docker**: 컨테이너 간 네트워크는 docker network로 자동 연결
