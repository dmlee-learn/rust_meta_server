# Phase 17 설계: PostgreSQL 재시도 + Observability 확장 + rquickjs 시그널

> Docker Compose timing 문제 해결, Prometheus histogram, structured logging, rquickjs C 라이브러리 시그널 처리.

---

## 1. PostgreSQL 연결 재시도

### D17-1: 재시도 로직
- `PostgresStore::connect()`에 재시도 추가
- 3회 시도, 2초 간격
- 실패 시 memory 스텁 폴백 (기존 동작 유지)

### D17-2: Redis 연결 재시도
- `RealRedisStore::connect()`에 재시도 추가
- 3회 시도, 2초 간격
- 실패 시 InMemoryPubSub 폴백 (기존 동작 유지)

---

## 2. Observability 확장

### D17-3: Prometheus Histogram
- `http_request_duration_ms` histogram (bucket: 1, 5, 10, 50, 100, 500, 1000ms)
- per-domain label: `board`, `erp`, `blog`, `shop`
- `meta_server_http_request_duration_ms_bucket`, `_sum`, `_count`

### D17-4: Structured Logging
- `RUST_LOG=json` 환경변수 시 JSON 형식 로그 출력
- 필드: `timestamp`, `level`, `message`, `domain`, `hook`, `elapsed_ms`
- 기본: 기존 텍스트 형식 유지

---

## 3. rquickjs C 라이브러리 시그널 문제

### D17-5: 별도 쓰레드에서 JS eval
- `SandboxedContext::eval_string()`을 별도 Rust 쓰레드에서 실행
- crash가 컨테이너 전체를 종료시키지 않도록 분리
- 구현: `std::thread::spawn` + `crossbeam-channel` 또는 `tokio::task::spawn_blocking`

### D17-6: Docker 시그널 처리
- Docker `--restart=always` 유지
- rquickjs 0.14 C 라이브러리 시그널 처리 버그 확인
- 새 버전 업그레이드 검토

---

## 4. 산출물

| 파일 | 내용 |
|------|------|
| `src/meta/store.rs` | PostgreSQL 연결 재시도 |
| `src/meta/redis.rs` | Redis 연결 재시도 |
| `src/metrics.rs` | Prometheus histogram |
| `src/observability.rs` | Structured logging (JSON) |
| `src/engine/runtime.rs` | 별도 쓰레드 JS eval |
| `tests/phase17_observability.rs` | Observability 테스트 |
