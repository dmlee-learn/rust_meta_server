# Phase 31 설계: 배포 자동화 (CI/CD)

> GitHub Actions CI/CD + Docker build/push + 환경 세팅.

---

## 1. CI 파이프라인 (`.github/workflows/ci.yml`)

### D31-1: Trigger
- `push` to `main`, `master`
- `pull_request` to `main`, `master`

### D31-2: Jobs
- **test**: cargo build + cargo test (Windows MSVC 또는 Linux)
  - Matrix: rust stable
  - Cache: cargo registry
- **lint**: cargo clippy
- **docker-build**: Docker image build (push 시)

### D31-3: Windows MSVC 특수성
- rquickjs는 MSVC + Windows SDK 필요
- CI에서 MSVC toolchain 설정 (MSYS2 또는 native MSVC)
- 대체안: Linux builder (rquickjs system library)

---

## 2. CD 파이프라인 (`.github/workflows/deploy.yml`)

### D31-4: Docker build + push
- `docker/build-push-action`
- Registry: GHCR (ghcr.io/OWNER/meta_server)
- Tag: `latest` + `sha-{short}` + `{version}`

### D31-5: Deploy
- Health check 기반 (외부 인프라 필요)
- 또는: manual approve + deploy job

---

## 3. 환경 세팅

### D31-6: `.env.example`
- `DATABASE_BACKEND`, `DATABASE_URL`
- `REDIS_ENABLED`, `REDIS_URL`
- `JWT_SECRET`, `JWT_DISABLED`
- `RATE_LIMIT_PER_MIN`
- `ADMIN_TOKEN`, `CORS_ORIGINS`
- `SOCKET_MAX_SESSIONS`

### D31-7: 환경 매트릭스
- dev: JWT_DISABLED=true, rate limit宽松
- staging: JWT enabled, 실 DB
- prod: JWT + admin token + 실 DB/Redis

---

## 4. 파일 변경

| 파일 | 변경 |
|------|------|
| `.github/workflows/ci.yml` | CI 파이프라인 |
| `.github/workflows/deploy.yml` | CD 파이프라인 |
| `.env.example` | 환경변수 예제 |

---

## 5. 검증

- YAML 구문 유효성 확인
- 기존 전체 테스트 통과
- .env.example 완성도
