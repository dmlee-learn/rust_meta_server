# HANDOVER: Phase 17 — PostgreSQL/Redis 재시도 + Prometheus histogram

> 이 문서는 Phase 17을 완료하고 단절했을 때, 다음 개발자(또는 다음 세션의 AI 에이전트)가
> 이어서 작업을 진행할 수 있도록 작성된 핸드오버 문서입니다.

---

## 1. Phase 17 완료 내용

### 1.1 PostgreSQL 연결 재시도
- `PostgresStore::connect()`: 3회 시도, 2초 간격
- 실패 시 memory 스텁 폴백 (기존 동작 유지)

### 1.2 Redis 연결 재시도
- `RealRedisStore::connect()`: 3회 시도, 2초 간격
- 실패 시 InMemoryPubSub 폴백 (기존 동작 유지)

### 1.3 Prometheus Histogram
- `http_request_duration_ms` histogram (per-domain: board, erp, blog, shop)
- bucket: 1, 5, 10, 50, 100, 500, 1000ms
- `Metrics::encode()` async 변경 (tokio::sync::RwLock 사용)

### 1.4 Docker Compose 실 검증
- PostgreSQL 연결 성공 (시도 1/3)
- Redis 연결 성공 (시도 1/3)
- 실 PostgreSQL INSERT 확인: `Phase 17 Test` board_posts에 저장
- 3개 컨테이너 모두 healthy

### 1.5 테스트
- 전체 96개 테스트 통과

---

## 2. 주의사항

### 2.1 Metrics::encode() async
`Metrics::encode()`가 Phase 17부터 async fn입니다.
`tokio::sync::RwLock`을 사용하므로 async 컨텍스트에서 `.await`가 필요합니다.

```rust
let text = metrics.encode().await;
```

### 2.2 PostgreSQL 연결 재시도
Docker Compose에서 PostgreSQL healthcheck가 통과한 후에도
연결 실패할 수 있습니다. 재시도 로직(3회, 2초 간격)이 이를 처리합니다.

### 2.3 CJ1104 Docker crash
rquickjs C 라이브러리 시그널 문제는 Phase 17에서 해결되지 않았습니다.
Docker `--restart=always`로 서비스 가용성을 유지합니다.

---

## 3. 다음 단계 (Phase 18)

### 3.1 rquickjs C 라이브러리 시그널 문제 해결
- 별도 쓰레드에서 JS eval 실행
- rquickjs 업그레이드 확인

### 3.2 Structured Logging
- `RUST_LOG=json` 환경변수 시 JSON 형식 로그 출력
- 필드: timestamp, level, message, domain, hook, elapsed_ms

### 3.3 Rate Limiting 확장
- per-domain rate limit
- user-based rate limit (JWT sub)
- burst limit

### 3.4 WebSocket 확장
- domain별 topic 구독
- real-time 유효화 알림

---

## 4. 빌드 및 테스트

```bash
# 전체 테스트 (96개)
cargo test

# Docker Compose 기동
cd /mnt/c/ai/meta_server
docker compose up -d

# API 호출
curl -X POST http://localhost:3000/api/board \
  -H "Content-Type: application/json" \
  -d '{"title":"Test","body":"Body"}'

# PostgreSQL 데이터 확인
docker exec meta_server-postgres-1 psql -U meta -d meta -c "SELECT * FROM board_posts;"

# Metrics 확인
curl http://localhost:3000/metrics | grep "duration"

# Docker Compose 종료
docker compose down
```
