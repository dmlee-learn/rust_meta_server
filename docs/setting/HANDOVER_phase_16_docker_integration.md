# HANDOVER: Phase 16 — Docker 실 검증 + 실 PostgreSQL/Redis 통합

> 이 문서는 Phase 16을 완료하고 단절했을 때, 다음 개발자(또는 다음 세션의 AI 에이전트)가
> 이어서 작업을 진행할 수 있도록 작성된 핸드오버 문서입니다.

---

## 1. Phase 16 완료 내용

### 1.1 Docker ERP/Blog/Shop crash 원인 진단

**증상:** Docker 컨테이너에서 `CJ1104`(JS throw) 에러가 발생하면 컨테이너가 crash (exit code 255).

**원인:** rquickjs C 라이브러리가 JS 예외(throw) 처리 시 시그널을 올바르게 처리하지 못함. Docker 컨테이너 환경에서 C 라이브러리 시그널 처리와 충돌.

**해결:**
- O16-1: Docker `--restart=always` (즉시 적용, 서비스 가용성 확보)
- O16-3: rquickjs 버그 보고 + 업그레이드 (장기 추적)

### 1.2 docker-compose.yml 확장

`meta_server` 서비스 추가:

```yaml
services:
  redis: ...
  postgres: ...
  meta_server:
    image: meta_server:latest
    restart: always
    depends_on:
      redis:
        condition: service_healthy
      postgres:
        condition: service_healthy
    ports:
      - "0.0.0.0:3000:3000"
    environment:
      DATABASE_BACKEND: postgres
      DATABASE_URL: postgres://meta:metapass@postgres:5432/meta
      REDIS_ENABLED: "true"
      REDIS_URL: redis://:metapass@redis:6379
      JWT_DISABLED: "true"
      RATE_LIMIT_PER_MIN: "100"
```

### 1.3 Docker 실 검증 결과

| 항목 | 결과 |
|------|------|
| Docker Compose 3개 컨테이너 | ✅ 모두 healthy |
| `/health` | ✅ `database.backend=postgres`, `redis.enabled=true` |
| `POST /api/board` | ✅ 201 Created |
| `POST /api/erp/purchase-orders` | ✅ 201 Created |
| `POST /api/blog/articles` (category 포함) | ✅ 201 Created |
| `POST /api/shop/orders` | ✅ 201 Created |
| 실 PostgreSQL INSERT | ✅ 확인 (`RUST_LOG=debug` 컨테이너) |
| 전체 테스트 | ✅ 91개 통과 |

### 1.4 미해결 이슈

| 이슈 | 설명 | 대응 |
|------|------|------|
| Docker Compose timing | PostgreSQL healthcheck 통과 후 meta_server 기동 시 연결 실패 → memory 폴백 | PostgreSQL 연결 재시도 로직 추가 (Phase 17) |
| CJ1104 Docker crash | JS throw 시 rquickjs C 라이브러리 시그널 문제 | `--restart=always` + rquickjs 업그레이드 |

---

## 2. 주의사항

### 2.1 Docker Compose PostgreSQL 연결

Docker Compose에서 `depends_on` + `condition: service_healthy`를 사용하지만,
PostgreSQL healthcheck가 통과한 후에도 meta_server 기동 시 연결 실패할 수 있습니다.
이는 PostgreSQL이 내부적으로 완전히 ready 상태가 아닐 수 있기 때문입니다.

**대응:**
- meta_server에 PostgreSQL 연결 재시도 로직 추가
- 또는 `start_period` 늘리기

### 2.2 CJ1104 Docker crash

rquickjs C 라이브러리가 JS throw 시 시그널을 올바르게 처리하지 못합니다.
Docker 컨테이너에서 이 시그널 처리 문제가 crash로 이어집니다.

**즉시 대응:** Docker `--restart=always`
**장기 대응:** rquickjs 업그레이드 + 버그 보고

### 2.3 실 PostgreSQL 데이터 확인

```bash
# Docker Compose 기동 후
docker compose -f docker-compose.yml up -d
sleep 10

# PostgreSQL 데이터 확인
docker exec meta_server-postgres-1 psql -U meta -d meta -c "SELECT * FROM board_posts;"
```

---

## 3. 다음 단계 (Phase 17)

### 3.1 PostgreSQL 연결 재시도 로직
- `PostgresStore::connect()`에 재시도 (3회, 2초 간격)
- Docker Compose timing 문제 해결

### 3.2 Observability 확장
- Prometheus histogram (latency distribution)
- per-domain metrics label
- structured logging (JSON)

### 3.3 API Rate Limiting 확장
- per-domain rate limit
- user-based rate limit (JWT sub)
- burst limit

### 3.4 rquickjs C 라이브러리 시그널 문제 해결
- rquickjs 업그레이드 확인
- 별도 쓰레드에서 JS eval 실행
- Docker 시그널 처리 설정

---

## 4. 빌드 및 테스트

```bash
# 전체 테스트 (91개)
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

# Docker Compose 종료
docker compose down
```
