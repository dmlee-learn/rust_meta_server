# Phase 4: Redis 실연동 + PostgreSQL 스키마

## 1. 개요

Phase 3(파이프라인 + 관측성) 완료 후, Redis Pub/Sub 실연동 코드와 PostgreSQL 스키마를 구현하는 단계.

**핵심 원칙:**
- Redis는 구현되지만 **기본 비활성** (REDIS_ENABLED=false)
- PostgreSQL은 **기본 백엔드** (DATABASE_BACKEND=postgres)
- 인스턴스 없을 때 자동 폴백 (InMemoryPubSub / MemoryStore)
- 전체 테스트가 인스턴스 없이 통과

## 2. 변경 파일

| 파일 | 변경 내용 |
|------|----------|
| `src/meta/redis.rs` | RealRedisStore 추가 (Pub/Sub, 캐시, 연결 폴백) |
| `src/meta/store.rs` | MetaStoreConfig에 redis 필드 추가, meta_store_config_from_env() |
| `src/main.rs` | 환경변수 기반 설정, Redis/L3 초기화, Pub/Sub 구독 루프 |
| `src/cache.rs` | L1Cache::invalidate() 추가 |
| `src/pipeline.rs` | build_board_pipeline_with_store() (L3 DataStore 기반) |
| `src/http.rs` | build_router가 Option<Arc<BoardPipeline>> 수신 |
| `migrations/001_init.sql` | PostgreSQL 스키마 (meta_modules + board_posts) |
| `tests/phase2_loader.rs` | async 테스트 갱신, RedisConfig::default() 사용 |

## 3. Redis 실연동 구조

```
RealRedisStore (redis crate)
├── cache_conn: Arc<MultiplexedConnection>  — 캐시 (GET/SETEX)
├── client: Client                         — Pub/Sub (into_pubsub)
│
├── get_cache(key)       → conn.get(key)
├── set_cache(entry)     → conn.set_ex(key, json, 3600)
├── publish_invalidation → PUBLISH meta:invalidation json
└── subscribe_loop       → PubSub::on_message() → handler(token)
```

**연결 폴백:**
- `REDIS_ENABLED=true` + 연결 성공 → `RealRedisStore`
- `REDIS_ENABLED=true` + 연결 실패 → `InMemoryPubSub` 스텁 (eprintln 경고)
- `REDIS_ENABLED=false` (기본) → `InMemoryPubSub` 스텁

## 4. 환경변수

| 변수 | 기본값 | 설명 |
|------|--------|------|
| `DATABASE_BACKEND` | `postgres` | `postgres` 또는 `memory` |
| `DATABASE_URL` | `postgres://user:pass@localhost:5432/meta` | PostgreSQL 연결 문자열 |
| `REDIS_ENABLED` | `false` | `true`/`false` |
| `REDIS_URL` | `redis://localhost:6379` | Redis 연결 문자열 |

## 5. 실행 방법

### 5.1 PostgreSQL 인스턴스 있을 때
```bash
# 스키마 적용
psql -U user -d meta -f migrations/001_init.sql

# 서버 기동
DATABASE_URL=postgres://user:pass@localhost:5432/meta \
  cargo run
```

### 5.2 Redis 인스턴스 있을 때
```bash
# Redis와 함께 기동
REDIS_ENABLED=true REDIS_URL=redis://localhost:6379 \
  DATABASE_BACKEND=memory \
  cargo run
```

### 5.3 인스턴스 없이 (개발/테스트)
```bash
# 기본: PostgreSQL 시도 → 실패 → MemoryStore 폴백
cargo run
```

## 6. 테스트

```bash
cargo test
# 전체 45개 통과 (Redis 비활성, PostgreSQL 인스턴스 없이)
```

## 7. 주의사항

### 7.1 Redis Pub/Sub 별도 연결
`MultiplexedConnection`은 `into_pubsub()`을 지원하지 않음.
Pub/Sub은 `Client::get_async_connection()`으로 별도 연결 사용.

### 7.2 redis crate `get_payload` 타입 추론 문제
`Msg::get_payload::<String>()`이 타입 추론 실패(`!` 타입)하는 경우 있음.
`get_payload_bytes()` → `std::str::from_utf8()`으로 변환하는 것이 안전.

### 7.3 `set_ex` 반환 타입
`AsyncCommands::set_ex`가 `RV: FromRedisValue`을 요구.
`let _: () = conn.set_ex(...).await...`으로 명시.

### 7.4 `Box<dyn FnMut>` 이동
`subscribe_loop`에서 `handler: Box<dyn FnMut>`를 받을 때 `mut handler` 필수.
호출 시 `(handler)(token)` 형태 (Box의 Deref).

### 7.5 PostgreSQL 연결 실패 시 폴백
`create_data_store`이 PostgreSQL 연결 실패 시, `main.rs`가 MemoryStore로 폴백.
에러 로그: `[MetaServer] L3 DataStore 연결 실패 → 메모리 스텁으로 폴백`

## 8. 다음 단계 (Phase 5)

- 소켓 통신 레이어 (`src/socket`)
- WebSocket 핸드셰이크 + on_socket_connect/message/disconnect 훅
- CR2001/CR2002/CJ2101 에러 코드 적용
