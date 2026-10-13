# Phase 18 설계: rquickjs 시그널 + Structured Logging + Rate Limiting 확장

> rquickjs C 라이브러리 시그널 문제 해결, JSON structured logging, per-domain rate limit.

---

## 1. rquickjs C 라이브러리 시그널 문제

### D18-1: 별도 쓰레드에서 JS eval
- `SandboxedContext::eval_string()`을 별도 Rust 쓰레드에서 실행
- crash가 컨테이너 전체를 종료시키지 않도록 분리
- `tokio::task::spawn_blocking` 사용 (async 컨텍스트에서 sync blocking 코드 실행)

### D18-2: Docker 시그널 처리
- Docker `--restart=always` 유지
- rquickjs 0.14 C 라이브러리 시그널 처리 버그 확인

---

## 2. Structured Logging (JSON)

### D18-3: JSON 로그 형식
- `RUST_LOG=json` 환경변수 시 JSON 형식 로그 출력
- 필드: `timestamp`, `level`, `message`, `domain`, `hook`, `elapsed_ms`, `error_code`
- 기본: 기존 텍스트 형식 유지

### D18-4: Observability 확장
- `Observability`에 `log_level` 필드 추가
- `hook_start`/`hook_end`/`hook_error` 시 JSON 로그 출력

---

## 3. Rate Limiting 확장

### D18-5: Per-domain rate limit
- domain별 rate limit 설정
- 예: `board=60`, `erp=30`, `blog=60`, `shop=60` (requests/minute)

### D18-6: Burst limit
- burst limit 추가 (단기간 내 급격한 요청 제한)
- 예: 10 requests/second burst limit

---

## 4. 산출물

| 파일 | 내용 |
|------|------|
| `src/engine/runtime.rs` | 별도 쓰레드 JS eval |
| `src/observability.rs` | JSON structured logging |
| `src/rate_limit.rs` | per-domain rate limit + burst limit |
| `src/http.rs` | per-domain rate limit 적용 |
| `tests/phase18_structured_logging.rs` | Structured logging 테스트 |
