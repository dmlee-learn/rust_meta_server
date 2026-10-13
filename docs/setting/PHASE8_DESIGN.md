# Phase 8 설계: 실 Redis/PostgreSQL 연동 + 운영 가이드 + API 문서

> 실 Redis + PostgreSQL 인스턴스를 Docker Compose로 기동하고,
> 실 환경에서 L1→L2(Redis)→L3(PG) 3단계 캐시 흐름을 실증한다.
> 운영 가이드와 API 문서를 완성해 생산 환경 배포를 준비한다.

---

## 1. 설계 결정

### D1: 실 인스턴스 기동 방식

**결정: Docker Compose로 Redis + PostgreSQL 기동**

- `docker-compose.yml`: `redis:7-alpine` + `postgres:15-alpine`
- 로컬 인스턴스 없이 Docker로 격리된 환경에서 실 연동 검증
- `migrations/001_init.sql`을 PostgreSQL 초기화 스크립트로 자동 실행

**Docker Compose 구성:**
```yaml
services:
  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"
    command: redis-server --requirepass metapass
    healthcheck:
      test: ["CMD", "redis-cli", "-a", "metapass", "PING"]
      interval: 5s
      timeout: 3s
      retries: 5

  postgres:
    image: postgres:15-alpine
    environment:
      POSTGRES_DB: meta
      POSTGRES_USER: meta
      POSTGRES_PASSWORD: metapass
    ports:
      - "5432:5432"
    volumes:
      - ./migrations/001_init.sql:/docker-entrypoint-initdb.d/001_init.sql
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U meta -d meta"]
      interval: 5s
      timeout: 3s
      retries: 5
```

**환경변수 (실 연동 시):**
```
REDIS_ENABLED=true
REDIS_URL=redis://:metapass@localhost:6379
DATABASE_BACKEND=postgres
DATABASE_URL=postgres://meta:metapass@localhost:5432/meta
```

### D2: 실 연동 테스트

**결정: `tests/phase8_real_integration.rs` + `#[ignore]`**

- Docker 인스턴스 필요 → `#[ignore]`로 기본 무시, 수동 실행
- 실 Redis 연결 확인 (PING)
- 실 PostgreSQL 연결 확인 (SELECT 1)
- L1 Miss → L2(Redis) Hit → L1 적재 실증
- L1 Miss → L2 Miss → L3(PG) → L2/L1 적재 실증
- Pub/Sub: `publish_invalidation` → L1 무효화 실증
- 게시판 글 저장 → PG에서 조회 실증

**실행 방법:**
```bash
docker compose up -d
cargo test --test phase8_real_integration -- --ignored --nocapture
docker compose down
```

### D3: 운영 가이드 문서

**결정: `docs/setting/OPERATION_GUIDE.md`**

내용:
1. 시스템 요구사항 (Rust, Docker, 환경변수)
2. 환경변수 정리 (DATABASE_BACKEND, DATABASE_URL, REDIS_ENABLED, REDIS_URL, SOCKET_MAX_SESSIONS)
3. Docker Compose 예제 + 실행 방법
4. 로컬 인스턴스 없이 스텁 모드 (기본값)
5. 실 인스턴스 활성화 (Redis + PostgreSQL)
6. 성능 튜닝 가이드 (SOCKET_MAX_SESSIONS, CPU_TIMEOUT_MS, MEMORY_LIMIT)
7. 에러 코드 표 (6자리 형식)
8. 모니터링 (Observability 이벤트, 로그 해석)
9. 캐시 무효화 흐름 (L1→L2→L3)

### D4: API 문서

**결정: `docs/setting/API.md`**

내용:
1. REST API: `POST /api/board` (요청/응답 JSON, 에러 코드)
2. REST API: `GET /health` (헬스체크)
3. WebSocket: `GET /ws` (upgrade, 메시지 프로토콜)
   - `on_socket_connect` → welcome
   - `on_socket_message` → reply
   - `on_socket_disconnect` → close
4. 에러 코드 표 (6자리, 도메인/레이어/타입)
5. 예시 (curl + WebSocket 클라이언트)

---

## 2. 산출물

| 파일 | 내용 |
|------|------|
| `docker-compose.yml` | Redis + PostgreSQL Docker Compose |
| `docs/setting/OPERATION_GUIDE.md` | 운영 가이드 |
| `docs/setting/API.md` | API 문서 (REST + WebSocket) |
| `tests/phase8_real_integration.rs` | 실 연동 테스트 (#[ignore]) |
| `docs/setting/PHASE8_DESIGN.md` | 이 문서 |
| `docs/STATUS.md` | Phase 8 완료 상태 갱신 |
| `docs/setting/HANDOVER_phase_8_real_integration.md` | Phase 8 핸드오버 |

---

## 3. 실 연동 테스트 상세

### phase8_real_integration.rs

```rust
// 테스트 1: redis_ping (#[ignore])
// - RealRedisStore::connect → PING 확인

// 테스트 2: postgres_connect (#[ignore])
// - PostgresStore::connect → SELECT 1 확인

// 테스트 3: l1_miss_l2_redis_hit (#[ignore])
// - 실 Redis에 module:base.board 적재
// - L1에서 get_module → L2(Redis) Hit → L1 적재

// 테스트 4: l1_miss_l2_miss_l3_pg (#[ignore])
// - 실 PostgreSQL에 meta_modules 저장
// - L1에서 get_module → L2(Redis) Miss → L3(PG) → L2/L1 적재

// 테스트 5: pubsub_invalidation (#[ignore])
// - 실 Redis Pub/Sub으로 publish_invalidation
// - L1 무효화 확인

// 테스트 6: board_post_save_pg (#[ignore])
// - 실 PostgreSQL에 board_posts 저장
// - get_board_post로 조회 확인
```

**전제:** Docker Compose로 Redis + PostgreSQL 기동된 상태.
테스트는 `#[ignore]`이므로 `cargo test` 기본 실행에서 제외.

---

## 4. 주의사항

1. **Docker 필요**: 실 연동 테스트는 Docker Compose 기동 필요
2. **기본 무시**: `#[ignore]`이므로 `cargo test` 기본 실행에 영향 없음
3. **기존 테스트 회귀 금지**: Phase 1~7의 55개 테스트 전부 통과
4. **에러 코드 6자리**: CR0004, CJ1102, CR2001, CJ2101 등
5. **문서 언어**: API 문서는 영어 (ASD-STE100), 운영 가이드는 한국어
