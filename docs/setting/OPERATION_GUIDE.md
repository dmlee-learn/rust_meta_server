# 운영 가이드 (Operation Guide)

> 본 시스템의 빌드, 실행, 모니터링, 성능 튜닝에 필요한 운영 지침.

---

## 1. 시스템 요구사항

| 항목 | 버전 |
|------|------|
| Rust | 1.75+ (stable) |
| Visual Studio Build Tools | 2022 (Windows) |
| Docker Desktop | 24+ (실 인스턴스 연동 시) |
| Redis | 7.x (Docker 또는 로컬) |
| PostgreSQL | 15.x (Docker 또는 로컬) |

---

## 2. 환경변수

| 변수 | 기본값 | 설명 |
|------|--------|------|
| `DATABASE_BACKEND` | `postgres` | 데이터 백엔드: `postgres` 또는 `memory` |
| `DATABASE_URL` | `postgres://user:pass@localhost:5432/meta` | PostgreSQL 연결 문자열 |
| `REDIS_ENABLED` | `false` | Redis(L2) 활성화: `true`/`false` |
| `REDIS_URL` | `redis://localhost:6379` | Redis 연결 문자열 |
| `SOCKET_MAX_SESSIONS` | `1000` | 최대 동시 WebSocket 세션 수 |
| `JWT_SECRET` | `dev-secret-change-me` | JWT HMAC 시크릿 |
| `JWT_DISABLED` | `false` | JWT 인증 비활성화: `true`/`false` |
| `RATE_LIMIT_PER_MIN` | `60` | 분당 최대 요청 수 (IP별) |
| `RUST_LOG` | `info` | 로그 레벨 |

### 2.1 스텁 모드 (기본값)

실 인스턴스 없이 메모리 스텁으로 동작:

```bash
# PostgreSQL 인스턴스 없이 메모리 스텁
set DATABASE_BACKEND=memory

# Redis 비활성 (InMemoryPubSub 스텁)
set REDIS_ENABLED=false

cargo run
```

### 2.2 실 인스턴스 모드

```bash
# PostgreSQL 활성화
set DATABASE_BACKEND=postgres
set DATABASE_URL=postgres://meta:metapass@localhost:5432/meta

# Redis 활성화
set REDIS_ENABLED=true
set REDIS_URL=redis://:metapass@localhost:6379

cargo run
```

---

## 3. Docker Compose (실 인스턴스)

### 3.1 구성

`docker-compose.yml`로 Redis + PostgreSQL 기동:

```bash
# 기동
docker compose up -d

# 확인 (health check)
docker compose ps

# 로그 확인
docker compose logs -f

# 종료
docker compose down
```

### 3.2 Docker 컨테이너 기동 (meta_server)

```bash
# meta_server 컨테이너 기동 (Docker Compose 네트워크에서)
export DATABASE_URL=postgres://meta:metapass@postgres:5432/meta
export REDIS_URL=redis://:metapass@redis:6379
export REDIS_ENABLED=true
export JWT_SECRET=production-secret-change-me

./target/release/meta_server
```

또는 `docker run`으로:

```bash
docker run -d \
  --name meta_server \
  --network docker_default \
  -p 3000:3000 \
  -e DATABASE_URL=postgres://meta:metapass@postgres:5432/meta \
  -e REDIS_URL=redis://:metapass@redis:6379 \
  -e REDIS_ENABLED=true \
  -e JWT_SECRET=production-secret \
  meta_server:latest
```

### 3.3 자동 초기화

PostgreSQL 컨테이너 기동 시 `migrations/001_init.sql`이 자동 실행:
- `meta_modules` 테이블 생성
- `board_posts` 테이블 생성

### 3.3 수동 스키마 적용 (기존 인스턴스)

```sql
-- PostgreSQL 클라이언트에서
\i migrations/001_init.sql
```

---

## 4. 빌드 및 실행

### 4.1 빌드

```bash
# Windows (MSVC)
source /c/temp/load_msenv.sh
export RUSTFLAGS='-C link-arg=/LIBPATH:C:\wsdk10\10.0.26100.0\um\x64 -C link-arg=/LIBPATH:C:\wsdk10\10.0.26100.0\ucrt\x64'
cargo build --release
```

### 4.2 실행

```bash
# 릴리스 빌드 실행
target/release/meta_server

# 개발 빌드 실행
cargo run
```

### 4.3 포트

- HTTP API: `http://0.0.0.0:3000`
- WebSocket: `ws://0.0.0.0:3000/ws`

---

## 5. 성능 튜닝

### 5.1 소켓 세션 풀

```
SOCKET_MAX_SESSIONS=1000   # 기본값
SOCKET_MAX_SESSIONS=5000   # 트래픽 증가 시
```

초과 시 `CR2001` (429 Too Many Requests) 반환.

### 5.2 rquickjs 샌드박스

| 파라미터 | 기본값 | 설명 |
|----------|--------|------|
| `memory_limit` | 16MB | 샌드박스 최대 메모리 |
| `cpu_timeout_ms` | 1500ms | CPU 타임아웃 (무한 루프 감지) |
| `max_stack` | 0 (무제한) | 스택 깊이 제한 |

### 5.3 캐시 TTL

- L1(Memory): 서버 실행 중 항상 유지 (무효화 시에만 삭제)
- L2(Redis): 1시간 (3600초) TTL
- L3(PostgreSQL): 영속

### 5.4 Rate Limiting

| 환경변수 | 기본값 | 설명 |
|----------|--------|------|
| `RATE_LIMIT_PER_MIN` | `60` | 분당 최대 요청 수 (IP별) |

초과 시 `CR4001` (429 Too Many Requests) + `Retry-After` 헤더 반환.

### 5.5 JWT 인증

| 환경변수 | 기본값 | 설명 |
|----------|--------|------|
| `JWT_SECRET` | `dev-secret-change-me` | HMAC 시크릿 (프로덕션에서 반드시 변경) |
| `JWT_DISABLED` | `false` | `true` 시 인증 없이 통과 |

토큰 형식: HS256, Claims: `sub`, `domains`(선택), `exp`

---

### 6.1 코어(C) — 인프라/레이아웃

| 코드 | 타입 | 설명 |
|------|------|------|
| `CR0001` | Infra | 시스템 메모리 부족 또는 Tokio 스레드 풀 할당 실패 |
| `CR0002` | Infra | rquickjs 엔진 초기화 실패 |
| `CR0004` | Infra | 모듈 미존재 (L3에서 모듈을 찾을 수 없음) |
| `CR0005` | Infra | 저장 실패 (DataStore 에러) |
| `CR2001` | Socket | 세션 풀 한계 초과 |
| `CR2002` | Socket | WebSocket 핸드셰이크 실패 |

### 6.2 스크립트(CJ) — 런타임

| 코드 | 타입 | 설명 |
|------|------|------|
| `CJ1101` | Runtime | JS 스크립트 구문 오류 (Syntax Error) |
| `CJ1102` | Runtime | CPU 타임아웃 (무한 루프) |
| `CJ1103` | Runtime | 메모리 한도 초과 |
| `CJ1104` | Runtime | 일반 JS 예외 (비즈니스 룰 위반) |
| `CJ2101` | Socket | 소켓 훅 직렬화/패킷 손실 |

### 6.3 상점(S) — 쇼핑몰

| 코드 | 타입 | 설명 |
|------|------|------|
| `SJ1201` | Runtime | 스크립트 계산 오류 (제목 필수 등) |
| `SJ1301` | Runtime | 고정 게시글 삭제 시도 |
| `SJ1401` | Runtime | 삭제 권한 없음 |

---

## 7. 모니터링

### 7.1 Observability 이벤트

서버 로그에서 `[OBS]` 접두사로 확인:

```
[OBS] start ext.notice v1
[OBS] end ext.notice v1 elapsed_ms=2.3
[OBS] SLOW ext.notice 612ms (threshold 500ms)
[OBS] ERROR ext.notice CJ1104: title is required
```

### 7.2 캐시 무효화 로그

```
[L1] 무효화: base.board v2
```

### 7.3 소켓 로그

```
[Socket] 세션 시작: <socket_id>
[Socket] 세션 종료: <socket_id>
```

---

## 8. 캐시 무효화 흐름

```
어드민 upsert_module (L3: PostgreSQL)
    │
    ▼
publish_invalidation(token) → Redis PUBLISH meta:invalidation
    │
    ▼ (모든 인스턴스 수신)
subscribe_invalidation → L1Cache.invalidate(module_id)
    │
    ▼
L1: module_id 제거 + L2: module:{module_id} 키 삭제
    │
    ▼ (다음 요청 시)
L1 Miss → L2(Redis) Miss → L3(PostgreSQL) 조회 → L2/L1 적재
```

---

## 9. 장애 대응

| 증상 | 원인 | 대응 |
|------|------|------|
| `CR0002` 스타트 업 실패 | rquickjs 엔진 초기화 실패 | 메모리/스택 확인 |
| `CR0004` 모듈 미존재 | L3에 모듈 없음 | `upsert_module` 호출 |
| `CJ1102` CPU 타임아웃 | 무한 루프 스크립트 | 스크립트 수정 |
| `CJ1103` 메모리 초과 | 샌드박스 한도 초과 | `memory_limit` 증가 |
| `CR2001` 세션 초과 | 동시 연결 초과 | `SOCKET_MAX_SESSIONS` 증가 |
| Redis 연결 실패 | Redis 인스턴스 없음 | `REDIS_ENABLED=false` 또는 인스턴스 기동 |
| PostgreSQL 연결 실패 | PG 인스턴스 없음 | `DATABASE_BACKEND=memory` 또는 인스턴스 기동 |
