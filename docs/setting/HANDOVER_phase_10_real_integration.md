# HANDOVER: Phase 10 — 실 Redis/PostgreSQL 연동

> 이 문서는 Phase 10을 완료하고 단절했을 때, 다음 개발자(또는 다음 세션의 AI 에이전트)가
> 이어서 작업을 진행할 수 있도록 작성된 핸드오버 문서입니다.

---

## 1. Phase 10 완료 내용

### 1.1 실 연동 테스트 (6개 전부 통과)

WSL2 Ubuntu에서 Docker Compose로 Redis + PostgreSQL 기동 후 실 연동 테스트를 실행했습니다.

| 테스트 | 내용 | 결과 |
|--------|------|------|
| `redis_ping` | 실 Redis PING + set/get/delete | ✅ OK |
| `postgres_connect` | 실 PostgreSQL 연결 + 모듈 upsert/get | ✅ OK |
| `l1_miss_l2_redis_hit` | L1 Miss → L2(Redis) Hit → L1 적재 | ✅ OK |
| `l1_miss_l2_miss_l3_pg` | L1 Miss → L2 Miss → L3(PG) → L2/L1 적재 | ✅ OK |
| `pubsub_invalidation` | Pub/Sub 무효화 + L1/L2 삭제 | ✅ OK |
| `board_post_save_pg` | 게시판 글 저장 → PG에서 조회 | ✅ OK |

**실행 방법:**
```bash
# Windows에서
wsl -d Ubuntu -- docker compose -f /mnt/c/ai/meta_server/docker-compose.yml up -d

# WSL2에서 테스트
wsl -d Ubuntu -- bash -c '
cd /mnt/c/ai/meta_server
source ~/.cargo/env
CC=gcc CXX=g++ cargo test --test phase8_real_integration -- --ignored --nocapture
'
```

### 1.2 PostgreSQL 스키마 수정

`migrations/001_init.sql`에서 `board_posts` 테이블 스키마를 `BoardPost` Rust 구조체에 맞게 수정:

```sql
-- 수정 전
CREATE TABLE board_posts (
    id          BIGINT PRIMARY KEY,
    title       TEXT NOT NULL,
    content     TEXT NOT NULL DEFAULT '',
    author      TEXT NOT NULL DEFAULT '',
    pinned      BOOLEAN NOT NULL DEFAULT FALSE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- 수정 후
CREATE TABLE board_posts (
    id          BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    title       TEXT NOT NULL,
    body        TEXT NOT NULL DEFAULT '',
    extra       TEXT NOT NULL DEFAULT 'null',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

### 1.3 PostgresStore 버그 수정

| 버그 | 원인 | 수정 |
|------|------|------|
| `error serializing parameter 5` | `version: i32`를 `bigint` 컬럼에 바인딩 | `i64`로 변환 후 바인딩 |
| `error deserializing column 5` | `version`을 `i32`로 읽음 | `i64`로 읽고 `as i32` |
| `created_at` 시리얼라이즈 | `i64` (epoch)를 `TIMESTAMPTZ`에 바인딩 | `to_timestamp($4::float8)` |
| `created_at` 디시리얼라이즈 | `TIMESTAMPTZ`를 `i64`로 읽음 | `EXTRACT(EPOCH FROM created_at)::BIGINT` |
| `board_posts.id` NULL | `id`가 `BIGINT PRIMARY KEY`인데 값 미지정 | `GENERATED ALWAYS AS IDENTITY` |

### 1.4 Docker Compose 수정

```yaml
# 수정 전
ports:
  - "6379:6379"

# 수정 후 (WSL2에서 Windows 접근을 위해 0.0.0.0 바인딩)
restart: always
ports:
  - "0.0.0.0:6379:6379"
```

### 1.5 API 문서 수정

- `POST /api/board` 성공 응답 코드: `200` → `201 Created`

### 1.6 전체 테스트: **61개 (Windows) + 6개 실 연동 (WSL2) 전부 통과**

---

## 2. 주의사항

### 2.1 WSL2에서 Docker 실행

Windows에서 Docker Desktop이 설치되어 있지 않아 WSL2 Ubuntu에서 Docker를 사용합니다.
WSL2에서 Docker 컨테이너의 포트를 Windows에서 접근하기 위해 `0.0.0.0:port:port`로 바인딩합니다.

### 2.2 PostgreSQL 스키마 재적용

`migrations/001_init.sql`을 수정한 후 기존 PostgreSQL 인스턴스에 적용하려면:
```sql
DROP TABLE IF EXISTS board_posts;
DROP TABLE IF EXISTS meta_modules;
-- 이후 001_init.sql 재실행
```
또는 `docker compose down -v` 후 재기동 (볼륨 삭제 시 자동 초기화).

### 2.3 실 연동 테스트는 `#[ignore]`

`phase8_real_integration.rs`의 6개 테스트는 `#[ignore]`로 기본 `cargo test`에서 제외됩니다.
Docker가 없는 CI 환경에서 실행되면 실패하므로 수동 실행합니다.

---

## 3. 다음 단계 (Phase 11)

### 3.1 추가 도메인 모듈
- `scripts/base/` + `scripts/extensions/`에 ERP, Blog, Shop 모듈
- AGENTS.md의 도메인 확장 스크립트

### 3.2 SDK 확장
- `src/sdk`에 추가 Rust 함수
- `rust_calculate_base_tax()`, `rust_send_notification()` 등

### 3.3 운영 배포
- Dockerfile 작성
- `cargo build --release`
- 실 환경 배포 가이드

---

## 4. 빌드 및 테스트

```bash
# 전체 테스트 (Windows, 61개)
cargo test

# 실 연동 테스트 (WSL2, Docker 필요)
wsl -d Ubuntu -- docker compose -f /mnt/c/ai/meta_server/docker-compose.yml up -d
wsl -d Ubuntu -- bash -c '
cd /mnt/c/ai/meta_server
source ~/.cargo/env
CC=gcc CXX=g++ cargo test --test phase8_real_integration -- --ignored --nocapture
'
```
