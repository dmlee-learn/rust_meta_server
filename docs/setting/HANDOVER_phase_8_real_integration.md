# HANDOVER: Phase 8 — 실 Redis/PostgreSQL 연동 + 운영 가이드 + API 문서

> 이 문서는 Phase 8을 완료하고 단절했을 때, 다음 개발자(또는 다음 세션의 AI 에이전트)가
> 이어서 작업을 진행할 수 있도록 작성된 핸드오버 문서입니다.

---

## 1. Phase 8 완료 내용

### 1.1 산출물

| 파일 | 내용 |
|------|------|
| `docker-compose.yml` | Redis 7 + PostgreSQL 15 Docker Compose |
| `tests/phase8_real_integration.rs` | 실 연동 테스트 6개 (`#[ignore]`) |
| `docs/setting/OPERATION_GUIDE.md` | 운영 가이드 (한국어) |
| `docs/setting/API.md` | API 문서 (영문 ASD-STE100) |
| `docs/setting/PHASE8_DESIGN.md` | Phase 8 설계 문서 |
| `docs/STATUS.md` | Phase 8 완료 상태 갱신 |

### 1.2 실 연동 테스트 (6개, `#[ignore]`)

| 테스트 | 내용 |
|--------|------|
| `redis_ping` | 실 Redis PING + set/get/delete 확인 |
| `postgres_connect` | 실 PostgreSQL 연결 + 모듈 upsert/get 확인 |
| `l1_miss_l2_redis_hit` | L1 Miss → L2(Redis) Hit → L1 적재 |
| `l1_miss_l2_miss_l3_pg` | L1 Miss → L2 Miss → L3(PG) → L2/L1 적재 |
| `pubsub_invalidation` | Pub/Sub 무효화 + L1/L2 삭제 확인 |
| `board_post_save_pg` | 게시판 글 저장 → PG에서 조회 |

**실행 방법:**
```bash
docker compose up -d
cargo test --test phase8_real_integration -- --ignored --nocapture
docker compose down
```

### 1.3 운영 가이드 주요 내용

- 환경변수 정리 (DATABASE_BACKEND, DATABASE_URL, REDIS_ENABLED, REDIS_URL, SOCKET_MAX_SESSIONS)
- 스텁 모드 vs 실 인스턴스 모드
- Docker Compose 기동/종료
- 성능 튜닝 (SOCKET_MAX_SESSIONS, memory_limit, cpu_timeout_ms)
- 에러 코드 표 (6자리)
- 모니터링 (Observability 이벤트, 로그 해석)
- 캐시 무효화 흐름
- 장애 대응 표

### 1.4 API 문서 주요 내용

- REST: `POST /api/board` (요청/응답 JSON, 에러 코드)
- REST: `GET /health`
- WebSocket: `GET /ws` (upgrade, 메시지 프로토콜)
  - on_socket_connect → welcome
  - on_socket_message → reply
  - on_socket_disconnect → close
- 에러 코드 표 (6자리, 도메인/레이어/타입)
- 예시 (curl + Node.js WebSocket)

---

## 2. 주의사항

### 2.1 Docker 필요

실 연동 테스트는 Docker Compose로 Redis + PostgreSQL 기동이 필요합니다.
`#[ignore]`이므로 기본 `cargo test`에서 제외됩니다.

### 2.2 PostgreSQL 초기화 스크립트

`docker-compose.yml`에서 `migrations/001_init.sql`을
`/docker-entrypoint-initdb.d/`에 마운트합니다.
PostgreSQL 컨테이너 **첫 기동 시에만** 실행됩니다.
기존 볼륨이 있으면 스키마가 자동 적용되지 않으므로,
`docker compose down -v` 후 재기동하거나 수동으로 실행해야 합니다.

### 2.3 Redis 비밀번호

Docker Compose에서 `redis-server --requirepass metapass`로 설정합니다.
`REDIS_URL=redis://:metapass@localhost:6379` 형식.

### 2.4 PostgreSQL 연결 문자열

`DATABASE_URL=postgres://meta:metapass@localhost:5432/meta`
- user: `meta`, password: `metapass`, db: `meta`

---

## 3. 다음 단계 (Phase 9)

### 3.1 실 연동 테스트 실행
- Docker Compose 기동 후 `cargo test --test phase8_real_integration -- --ignored`
- 실 Redis/PostgreSQL 환경에서 L1→L2→L3 캐시 실증
- Pub/Sub 무효화 실증

### 3.2 추가 도메인 모듈
- `scripts/base/` + `scripts/extensions/`에 추가 도메인 모듈
- ERP, Blog, Shop 등 AGENTS.md의 도메인 확장

### 3.3 SDK 확장
- `src/sdk`에 추가 Rust 함수 자산
- `rust_calculate_base_tax()`, `rust_send_notification()` 등

### 3.4 멀티인스턴스 테스트
- 2개 서버 인스턴스 기동
- 실 Redis Pub/Sub으로 L1 무효화 실증
- L1 캐시 정합성 검증

---

## 4. 빌드 및 테스트

```bash
# 전체 테스트 (Phase 8 6개 제외, 55개)
cargo test

# Phase 8 실 연동 테스트 (Docker 필요)
docker compose up -d
cargo test --test phase8_real_integration -- --ignored --nocapture
docker compose down
```
