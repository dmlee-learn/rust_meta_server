# HANDOVER: Phase 31 — 배포 자동화 (CI/CD)

> 이 문서는 Phase 31를 완료하고 단절했을 때, 다음 개발자(또는 다음 세션의 AI 에이전트)가
> 이어서 작업을 진행할 수 있도록 작성된 핸드오버 문서입니다.

---

## 1. Phase 31 완료 내용

### 1.1 CI 워크플로우 (`.github/workflows/ci.yml`)
- **test job** (Windows MSVC):
  - MSYS2 (MINGW64) + mingw-w64-x86_64-gcc 설치
  - Rust 설치 + cargo cache
  - `RUSTFLAGS=/LIBPATH:...` (rquickjs MSVC lib path)
  - `cargo build --release` + `cargo test --release`
- **lint job** (Linux):
  - `cargo clippy --all-targets -- -D warnings`

### 1.2 CD 워크플로우 (`.github/workflows/deploy.yml`)
- Docker build + push to GHCR
- Tags: `latest`(default branch) + `sha-{short}` + `semver`
- Trigger: push to main/master + tags `v*.*.*` + manual

### 1.3 `.env.example`
- BIND_ADDR, DATABASE_BACKEND, DATABASE_URL
- REDIS_ENABLED, REDIS_URL
- JWT_DISABLED, JWT_SECRET, JWT_ISSUER, JWT_TTL_SECONDS
- RATE_LIMIT_PER_MIN
- ADMIN_TOKEN, CORS_ORIGINS
- SOCKET_MAX_SESSIONS, RUST_LOG

---

## 2. 주의사항

### 2.1 Windows CI 특수성
- rquickjs는 MSVC C 라이브러리 필요 → mingw-w64-x86_64-gcc 사용
- RUSTFLAGS `/LIBPATH`는 CI environment path에 맞춤
- mingw-w64 라이브러리 경로: `C:\msys64\x86_64-w64-mingw32\lib`
- 만약 CI에서 rquickjs 링크 실패: MSVC native 또는 Linux builder로 전환

### 2.2 GHCR 권한
- `permissions.packages: write` 필요
- private repo면 registry secret 필요
- `GITHUB_TOKEN`은 push에 자동 포함

### 2.3 Docker build 경로
- `context: .` → 프로젝트 루트
- Dockerfile은 `COPY . .`로 모든 파일 포함
- `.dockerignore`로 target/node_modules 제외 확인

### 2.4 clippy -D warnings
- warning이 error로 처리 → CI 실패
- 신규 warning 발생 시 `-D` 해제 또는 warning 수정 필요

---

## 3. 다음 단계 (Phase 32)

### 3.1 Observability 강화
- OpenTelemetry distributed tracing
- Structured logging enrichment
- SLO/SLI 정의 (error budget, latency target)
- Prometheus alert rules

### 3.2 성능 최적화 (고급)
- rquickjs warm-up pool (pre-allocated contexts)
- Connection pool tuning
- Query optimization

---

## 4. 사용법

```bash
# .env 세팅
cp .env.example .env
# DATABASE_BACKEND=postgres, JWT_SECRET, etc.

# CI/CD는 GitHub push 시 자동 실행
git push origin main

# Docker image 확인
ghcr.io/{owner}/meta_server:latest
```
