# 개발 진행 상태 (Development Status)

> 이 문서는 개발을 **중단했다가 이어서 진행할 수 있도록** 현재 위치, 다음 액션, 완료 기준을 기록하는 문서입니다.
> 누구라도 이 문서의 `[WIP 체크포인트]`와 `[진행 지침]`만 읽고 작업을 이어서 수행할 수 있어야 합니다.

---

## 1. WIP 체크포인트 (Work In Progress Checkpoint)

> 현재 작업 중인 Task의 **정확한 중단 위치**를 기록합니다.
> 단절 시 "다음 개발자"가 이 섹션을 읽고 바로 작업을 이어갈 수 있게 작성해야 합니다.

### 현재 상태
- **진행 중 Task:** `(Phase 42 P1-P5 전부 완료 — 시스템 전체 완료)`
- **마지막 작업 시점:** `2026-10-12 (Phase 42 P1 Docker 실서버 검증 + 기능별 테이블 스크립트 + blog category 수정. 138개 테스트 통과, Docker 실서버 검증 완료)`
- **중단 위치 (File/Line/Function):** `(Step 1 완료 - Step 2 시작 대기)`
- **다음 액션 (Next Action):**
  1. Phase 41 Step 2: 스태킹/인스턴스 생성 API (`/api/{domain}/items?template_id=`) + Q3 (bump/propagate/sync_policy) + 사이트 템플릿 프로비저닝 + 대시보드 UI + OpenAPI
  2. ⚠️ `POST /api/admin/templates/{id}/bump` = 새 버전 (기존 인스턴스 스냅샷 보존), `propagate` = `sync_policy='live'` 인스턴스만 payload 재적용
  3. ⚠️ 기존 도메인 테이블(`board_posts`/`blog_articles`/`shop_products`)에 `template_id` 컬럼 추가됨 (Phase 41 migration 006)
  4. ⚠️ 템플릿 트리 결합은 `build_tree_inner`(Box::pin) 순환 참조 방지 — (template_id,version) HashSet 추적
  5. ⚠️ PostgreSQL 신규 쿼리는 `docs/setting/HANDOVER_phase_38_shop.md` 섹션 4의 안전한 직렬화 패턴 준수

### 중단 시 기록 형식 (전용 템플릿)

작업을 중단할 때 아래 형식으로 위 섹션을 갱신하세요:

```
### 현재 상태
- 진행 중 Task: P1-1.2 (rquickjs 런타임 초기화)
- 마지막 작업 시점: 2026-09-30 14:30
- 중단 위치: src/engine/mod.rs, Runtime::new() 함수 내 42번째 라인
- 다음 액션:
  1. Runtime::new()에 MemoryLimit 옵션 적용
  2. Throttling 테스트 케이스 추가 (tests/engine_throttling.rs)
- 주의사항:
  - rquickjs 0.x의 MemoryLimit API는 Option<u64>을 받음
  - 기존 코드에서 unwrap() 사용 금지 (AGENTS.md 규약)
```

---

## 2. Phase별 진행 상태 (Phase Status)

### Phase 1: 인프라 뼈대
- **1.1 서버 스캐폴딩:** ✅ 완료 (src/main.rs, src/http.rs, Axum 라우터)
- **1.2 런타임 초기화:** ✅ 완료 (src/engine/runtime.rs, Engine + SandboxedContext)
- **1.3 리소스 제한:** ✅ 완료 (CPU Timeout→CJ1102, Memory Limit→CJ1103, JS throw→CJ1104)
- **1.4 테스트:** ✅ 완료 (유닛 5 + 기능 5 + 통합 7 = 17개 전부 통과)
- **1.5 검증:** ✅ 완료 (포트 3999로 HTTP 엔드포인트 실측)
- **1.6 핸드오버:** ✅ 완료 (docs/setting/HANDOVER_phase_1_engine_throttling.md)

### Phase 2: 바인딩 레이어 + 데이터 레이어
- **2.1 DataStore:** ✅ 완료 (src/meta/store.rs — PostgreSQL 기본 + 메모리 스텁 폴백)
- **2.2 Redis 스텁:** ✅ 완료 (src/meta/redis.rs — 기능 구현, 기본 비활성화)
- **2.3 모듈 로딩:** ✅ 완료 (src/engine/loader.rs — Base→Extension + CJ1101)
- **2.4 샘플 스크립트:** ✅ 완료 (scripts/base/board.js, scripts/extensions/notice.js)
- **2.5 테스트:** ✅ 완료 (tests/phase2_loader.rs — 10개)
- **2.6 핸드오버:** ✅ 완료 (docs/setting/HANDOVER_phase_2_datastore_loader.md)

### Phase 3: 파이프라인 체이닝 + 관측성
- **3.1 L1 캐시 적재:** ✅ 완료 (src/cache.rs — Single Flight 패턴, 무효화)
- **3.2 훅 발동 로직:** ✅ 완료 (src/pipeline.rs — BoardPipeline + before_create/after_save)
- **3.3 핸들러 파이프라인:** ✅ 완료 (src/http.rs — /api/board 엔드포인트)
- **3.4 관측성:** ✅ 완료 (src/observability.rs — ExecutionReport + Event Queue)
- **3.5 에러 매핑:** ✅ 완료 (LoadError/SandboxError → CJ11xx JSON)
- **3.6 테스트:** ✅ 완료 (tests/phase3_pipeline.rs — 6개)
- **3.7 핸드오버:** ✅ 완료 (docs/setting/HANDOVER_phase_3_pipeline_observability.md)

### Phase 4: Redis 실연동 + PostgreSQL 스키마
- **4.1 Redis 실연동:** ✅ 완료 (src/meta/redis.rs — RealRedisStore, Pub/Sub, 연결 폴백)
- **4.2 Redis 설정:** ✅ 완료 (REDIS_ENABLED/REDIS_URL 환경변수, 기본 비활성)
- **4.3 PostgreSQL 스키마:** ✅ 완료 (migrations/001_init.sql)
- **4.4 메타스토어 설정:** ✅ 완료 (DATABASE_BACKEND/DATABASE_URL 환경변수)
- **4.5 테스트:** ✅ 완료 (전체 45개 통과 — Phase 2/3 테스트 갱신)

### Phase 5: 소켓 통신 레이어
- **5.1 소켓 모듈:** ✅ 완료 (src/socket/ — config, session, handler, ws)
- **5.2 SessionPool:** ✅ 완료 (소켓 세션 풀, `SOCKET_MAX_SESSIONS` 환경변수)
- **5.3 WebSocket 핸들러:** ✅ 완료 (axum WebSocket + `tokio-tungstenite`)
- **5.4 JS 훅 발동:** ✅ 완료 (`on_socket_connect/message/disconnect` → rquickjs)
- **5.5 샘플 스크립트:** ✅ 완료 (scripts/socket_hooks.js)
- **5.6 테스트:** ✅ 완료 (tests/phase5_socket.rs — 3개, 전체 48개 통과)

### Phase 6: 다층 캐시 연동 + 성능 벤치마크
- **6.1 L1 무효화 시나리오:** ✅ 완료 (tests/phase6_cache_flow.rs — 2개)
- **6.2 성능 벤치마크:** ✅ 완료 (tests/phase6_benchmark.rs — 5개)
  - L1 캐시 적중률: 100% (기준 90% 이상)
  - 훅 발동 성능: 1.00ms/회 (기준 10ms 미만)
  - 무한 루프 감지: 500.08ms (기준 600ms 미만)
  - 동시 WebSocket: 50/50 성공
  - 세션 풀 한계: 5 성공 + 1 CR2001
- **6.3 전체 테스트:** ✅ 완료 (55개 전부 통과 — Phase 1~6)

---

## 3. 완료 기준 (Acceptance Criteria)

> 각 Task 완료 시 아래 체크리스트를 모두 만족해야 Phase를 다음 단계로 진행할 수 있습니다.

### Phase 1 완료 기준
- [x] Axum 서버가 지정 포트(3999)에서 응답을 반환한다.
- [x] Tokio 멀티스레드 런타임이 정상 활성화된다.
- [x] rquickjs 컨텍스트가 생성될 때 CPU Timeout(1000ms)이 강제 적용된다.
- [x] rquickjs 컨텍스트가 생성될 때 Memory Limit(8MB)이 강제 적용된다.
- [x] 제한 초과 시 CJ1102/CJ1103 에러가 반환되고 서버가 크래시하지 않는다.
- [x] JS 일반 throw 시 CJ104가 반환된다.

### Phase 2 완료 기준
- [x] `DataStore` 추상 트레잇으로 PostgreSQL(기본)과 메모리 스텁을 투명하게 교환.
- [x] Redis 기능(캐시/PubSub)을 구현하되 기본 비활성화.
- [x] `scripts/base`→`extensions` 모듈 로딩 파이프라인 (Base→Extension 상속).
- [x] CJ1101(구문오류)이 캐시 적재 단계에서 감지되어 로딩 중단.
- [x] 모든 스텁/로더 코드가 `unwrap`/`panic!` 없이 `Result`를 반환.

### Phase 2 추가 메모
- **의존성**: `tokio-postgres` 0.7.18, `async-trait` 0.1.92, `thiserror` 2.0.21
- **주의**: Base→Extension 바인딩에서 템플릿 `var api` 재선언 금지 (프로토타입 충돌) — HANDOVER §1.3

### Phase 3 완료 기준
- [x] 게시판 글 생성 요청 시 L1 캐시의 스크립트 `before_create` 훅이 DB 저장 직전에 실행된다.
- [x] 훅 통과 시 DB에 저장되고 성공 응답을 반환한다.
- [x] 훅 실패 시 SJ1201 등 에러 코드 JSON을 반환하고 DB 저장이 차단된다.
- [x] 무한 루프 스크립트 테스트 시 CPU Timeout으로 강제 종료되고 CJ1102가 반환된다.

### Phase 4 완료 기준
- [x] Redis 실연동 코드 구현 (RealRedisStore: Pub/Sub, 캐시).
- [x] Redis 비활성 시 InMemoryPubSub 스텁으로 자동 폴백.
- [x] Redis 연결 실패 시 InMemoryPubSub 스텁으로 자동 폴백.
- [x] PostgreSQL 스키마 DDL (meta_modules + board_posts).
- [x] 환경변수 기반 설정 (DATABASE_BACKEND, DATABASE_URL, REDIS_ENABLED, REDIS_URL).
- [x] 전체 테스트 45개 통과 (Redis 비활성, PostgreSQL 인스턴스 없이).

### Phase 5 완료 기준
- [x] axum WebSocket upgrade가 정상 동작한다 (`/ws` 엔드포인트).
- [x] `on_socket_connect` 훅 발동 시 welcome 메시지를 클라이언트에 반환한다.
- [x] `on_socket_message` 훅 발동 시 reply를 클라이언트에 반환한다.
- [x] `on_socket_message` 훅에서 JS throw 시 CJ2101 에러 코드를 반환한다.
- [x] 세션 풀 제한(`SOCKET_MAX_SESSIONS`) 초과 시 CR2001을 반환한다.
- [x] `on_socket_disconnect` 훅 발동 시 연결 종료 후 세션을 정리한다.
- [x] 전체 테스트 48개 통과 (Phase 1~5).

### Phase 6 완료 기준
- [x] L1 무효화 → L3 새 버전 upsert → L1 재적재 시나리오 테스트 통과.
- [x] L1 캐시 적중률 90% 이상 (실측 100%).
- [x] 훅 발동 성능 10ms 미만 (실측 1.00ms/회).
- [x] 무한 루프 감지 시간 cpu_timeout_ms + 100ms 미만 (실측 500.08ms, cpu_timeout_ms=500).
- [x] 동시 WebSocket 50개 연결 성공 (50/50).
- [x] 세션 풀 한계(`SOCKET_MAX_SESSIONS=5`) 초과 시 CR2001 반환 (5 성공 + 1 실패).
- [x] 전체 테스트 55개 통과 (Phase 1~6).

### Phase 7 완료 기준
- [x] L1 Miss → L2(Redis) 조회 → Hit 시 L1 적재.
- [x] L1 Miss → L2 Miss → L3(PG) 조회 → L2/L1 적재.
- [x] L2 무효화(`invalidate`) 시 L2에서 `module:{module_id}` 키 삭제.
- [x] 동시 100개 WebSocket 연결 성공 (100/100).
- [x] L1 적중률 90% 이상 (실측 99.0%).
- [x] L2 캐시 조회 성능 5ms 미만 (실측 0.000ms/회, InMemoryPubSub).
- [x] 전체 테스트 55개 통과 (Phase 1~7).

### Phase 8 완료 기준
- [x] Docker Compose 구성 (Redis 7 + PostgreSQL 15).
- [x] 실 연동 테스트 6개 작성 (`#[ignore]`, Docker 필요).
- [x] 운영 가이드 문서 (`docs/setting/OPERATION_GUIDE.md`).
- [x] API 문서 (`docs/setting/API.md`, 영문 ASD-STE100).
- [x] 전체 테스트 55개 통과 (Phase 1~8, Phase 8 6개는 `#[ignore]`).

### Phase 9 완료 기준
- [x] 멀티인스턴스 L1 무효화 시뮬레이션 (InMemoryPubSub 공유, 2개 BoardPipeline).
- [x] 인스턴스 A 무효화 시 L2(공유 InMemoryPubSub)에도 삭제.
- [x] L3에 새 버전 upsert → 두 인스턴스 모두 새 버전 재적재.
- [x] 동시 50개 요청 시 Single Flight 동작.
- [x] API 문서 201 Created 수정 (기존 200 → 201).
- [x] 전체 테스트 61개 통과 (Phase 1~9, Phase 8 6개는 `#[ignore]`).

### Phase 10 완료 기준
- [x] 실 Redis + PostgreSQL 연동 (WSL2 Docker Compose).
- [x] 실 연동 테스트 6개 전부 통과 (redis_ping, postgres_connect, l1_miss_l2_redis_hit, l1_miss_l2_miss_l3_pg, pubsub_invalidation, board_post_save_pg).
- [x] PostgreSQL 스키마 수정 (board_posts: body/extra 컬럼, GENERATED ALWAYS AS IDENTITY).
- [x] PostgresStore 버그 수정 (version: i32→i64, created_at: to_timestamp/EXTRACT(EPOCH)).
- [x] Docker Compose 포트 바인딩 수정 (0.0.0.0:port:port, restart: always).
- [x] API 문서 201 Created 수정.
- [x] 전체 테스트 61개 통과 (Windows) + 실 연동 6개 통과 (WSL2).

### Phase 11 완료 기준
- [x] `src/sdk/math.rs`: `rust_calculate_base_tax()`, `rust_calculate_discount()` 구현.
- [x] `src/sdk/utils.rs`: `rust_hash_token()`, `rust_validate_uuid()` 구현.
- [x] `SandboxedContext::create_sandbox()`에 SDK 4개 함수 바인딩 (`rquickjs::Function::new`).
- [x] `tests/phase11_sdk_domain.rs`: 6개 테스트 (SDK 단위, JS 바인딩, ERP/Shop/Blog 모듈).
- [x] 전체 테스트 73개 통과 (기존 67개 + Phase 11 6개).

### Phase 12 완료 기준
- [x] `Dockerfile`: 멀티 스테이지 빌드 (rust:1.79-slim → debian:bookworm-slim).
- [x] `.dockerignore`: 빌드 제외 파일.
- [x] `src/domain.rs`: DomainPipeline 추상화 (도메인별 Base+Extension + 훅 + DataStore).
- [x] `src/meta/store.rs`: ERP/Blog/Shop 엔티티 + DataStore 메서드 추가.
- [x] `migrations/002_domain_tables.sql`: ERP/Blog/Shop 테이블 스키마.
- [x] `src/http.rs`: `/api/erp/purchase-orders`, `/api/blog/articles`, `/api/shop/orders` 엔드포인트.
- [x] `src/main.rs`: ERP/Blog/Shop 파이프라인 기동.
- [x] `tests/phase12_domain.rs`: 4개 테스트 (ERP/Blog/Shop 파이프라인 + DataStore).
- [x] 전체 테스트 77개 통과 (기존 73개 + Phase 12 4개).

### Phase 13 완료 기준
- [x] `src/meta/migrate.rs`: 마이그레이션 자동 적용 (`migrations/*.sql` 순차 적용).
- [x] `PostgresStore::connect()`: MySQL/PostgreSQL 연결 시 마이그레이션 자동 적용.
- [x] `src/http.rs`: `/health` 엔드포인트 확장 (DB/Redis/파이프라인/소켓 상태).
- [x] `SessionPool::len()`: sync 활성 세션 수 조회 (AtomicUsize).
- [x] `Dockerfile`: HEALTHCHECK 추가.
- [x] `build_router` 시그니처 확장 (db_backend, migrations, redis_enabled).
- [x] `tests/phase13_migrate.rs`: 3개 테스트 (마이그레이션, health, 세션 풀).
- [x] 전체 테스트 80개 통과 (기존 77개 + Phase 13 3개).

### Phase 14 완료 기준
- [x] `src/metrics.rs`: Prometheus text format metrics (HTTP 요청, 캐시, 훅, 세션, 엔진 메모리).
- [x] `src/auth.rs`: JWT 인증 (Bearer 토큰 검증, 도메인별 권한 체크).
- [x] `src/http.rs`: `/metrics` 엔드포인트 + JWT 미들웨어 적용.
- [x] `build_router` 시그니처 확장 (jwt_secret, jwt_disabled).
- [x] `tests/phase14_metrics_auth.rs`: 4개 테스트 (metrics encode, JWT 생성/검증, 만료, build_router).
- [x] 전체 테스트 87개 통과 (기존 83개 + Phase 14 4개).

### Phase 15 완료 기준
- [x] `src/rate_limit.rs`: IP 기반 sliding window rate limiting (분당 60회, 429 + Retry-After).
- [x] `src/http.rs`: Rate Limiting 미들웨어 적용 + `rate_limiter` 필드.
- [x] `build_router` 시그니처 확장 (rate_limit_per_minute).
- [x] `docs/setting/API.md`: JWT 인증, Rate Limiting, `/metrics`, ERP/Blog/Shop API 추가.
- [x] `docs/setting/OPERATION_GUIDE.md`: JWT/Rate Limiting 환경변수, Docker 컨테이너 기동 절차 추가.
- [x] `tests/phase15_rate_limit.rs`: 4개 테스트.
- [x] Docker 이미지 빌드 성공 (rust:1.88-slim).
- [x] Docker 실 검증: `/health`, `/metrics`, `POST /api/board` 정상.
- [x] 전체 테스트 91개 통과 (기존 87개 + Phase 15 4개).
- [ ] Docker 환경에서 ERP/Blog/Shop API crash 문제 (별도 이슈).

### Phase 16 완료 기준
- [x] Docker ERP/Blog/Shop crash 원인 진단: CJ1104(JS throw) 시 rquickjs C 라이브러리 시그널 처리 문제.
- [x] `docker-compose.yml`: meta_server 서비스 추가 (PostgreSQL + Redis + meta_server 동시 기동).
- [x] Docker 실 검증: `RUST_LOG=debug` 컨테이너에서 실 PostgreSQL INSERT 확인.
- [x] Docker Compose: 3개 컨테이너( redis, postgres, meta_server) 동시 healthy 기동.
- [x] `/health`: `database.backend=postgres`, `redis.enabled=true` 확인.
- [x] `POST /api/board`, `/api/erp/purchase-orders`, `/api/shop/orders` Docker Compose 정상.
- [x] 전체 테스트 91개 통과.
- [ ] Docker Compose timing: PostgreSQL healthcheck 통과 후 meta_server 기동 시 연결 실패 (Phase 17에서 재시도 로직으로 해결).
- [ ] CJ1104 에러 시 Docker crash (rquickjs C 라이브러리 문제, 별도 이슈).

### Phase 17 완료 기준
- [x] PostgreSQL 연결 재시도: 3회 시도, 2초 간격.
- [x] Redis 연결 재시도: 3회 시도, 2초 간격.
- [x] Prometheus histogram: `http_request_duration_ms` (per-domain).
- [x] `Metrics::encode()` async 변경 (histogram 읽기용).
- [x] Docker Compose 실 검증: PostgreSQL/Redis 연결 성공 (시도 1/3).
- [x] 실 PostgreSQL INSERT 확인: `Phase 17 Test` board_posts에 저장.
- [x] 전체 테스트 96개 통과.

### Phase 18 완료 기준
- [x] JSON structured logging: `LoggingObserver::json()` (RUST_LOG=json 시 사용).
- [x] Per-domain rate limit: `RATE_LIMIT_BOARD_PER_MIN`, `RATE_LIMIT_ERP_PER_MIN`, etc.
- [x] `extract_domain_from_path()` — path에서 domain 추출.
- [x] Docker Compose 실 검증: 3개 컨테이너 healthy, PostgreSQL INSERT 확인.
- [x] 전체 테스트 96개 통과.

### Phase 19 완료 기준
- [x] User-based rate limit: `RateLimiter::check_user()` (JWT sub 기반).
- [x] `extract_user_from_request()` — JWT sub 추출.
- [x] `decode_token_claims()` — auth.rs에 추가.
- [x] 에러 코드: `CR4003` (user rate limit).
- [x] Docker Compose 실 검증: 3개 컨테이너 healthy, PostgreSQL INSERT 확인 (board + erp).
- [x] 전체 테스트 96개 통과.

### Phase 20 완료 기준
- [x] `with_observer_eval()` async 변경 (`.await` 추가).
- [x] Docker Compose 실 검증: 3개 컨테이너 healthy, board/erp/blog 전부 정상.
- [x] 컨테이너 crash 없음 (async eval 변경 후에도 정상).
- [x] 전체 테스트 96개 통과.

### Phase 21 완료 기준
- [x] WebSocket domain query parameter: `?domain=erp`.
- [x] Grafana provisioning: datasource + dashboard.
- [x] Prometheus config: `prometheus.yml`.
- [x] Docker Compose: 5개 컨테이너 (redis, postgres, meta_server, prometheus, grafana).
- [x] OpenAPI 3.0 spec: `docs/openapi.yaml`.
- [x] Docker Compose 실 검증: 5개 컨테이너 기동, API 정상.
- [x] 전체 테스트 96개 통과.

### Phase 22 완료 기준
- [x] `src/admin.rs`: 대시보드 HTML (Chart.js, dark theme, CSS Grid).
- [x] `GET /admin`: 대시보드 페이지 서빙.
- [x] `GET /api/admin/status`: 종합 서버 상태 JSON (uptime, engine, db, redis, pipelines, socket, auth).
- [x] `GET /api/admin/metrics`: 메트릭스 JSON 스냅샷 (http_requests, cache, hooks, engine_memory).
- [x] `rate_limit_middleware`: `record_http_request` 호출 추가.
- [x] `/admin`, `/api/admin/*`: JWT/Rate Limit 미들웨어 제외.
- [x] Docker Compose 실 검증: 대시보드 HTML 서빙 + API 정상 + metrics 카운트 확인.
- [x] 전체 테스트 96개 통과.

### Phase 23 완료 기준
- [x] `GET /ws/admin`: 5초 간격 metrics JSON push WebSocket.
- [x] `GET /api/admin/logs`: 마지막 50개 에러 로그 조회.
- [x] `POST /api/admin/cache/flush`: 모든 L1 캐시 flush (4개: board+erp+blog+shop).
- [x] `GET /api/admin/ping`: 가벼운 서버 생존 확인.
- [x] `AppState.error_logs`: `Mutex<VecDeque<ErrorLogEntry>>` 링 버퍼 (최대 100개).
- [x] 대시보드 HTML: WebSocket 우선 + polling 폴백, 에러 로그 패널, 캐시 flush 버튼.
- [x] Docker Compose 실 검증: ping, logs, flush, admin HTML, metrics 전부 정상.
- [x] 전체 테스트 96개 통과.

### Phase 24 완료 기준
- [x] `AppState::record_error()` 메서드: 에러 로그 기록 (code, message, domain, timestamp).
- [x] `board_create`: 에러 발생 시 `record_error()` + `metrics.record_hook_error()` 호출.
- [x] `handle_domain`: 에러 발생 시 `record_error()` + `metrics.record_hook_error()` 호출.
- [x] CR0005 (파이프라인 미기동)도 에러 로그에 기록.
- [x] Docker 실 검증: CJ1104 에러 2개 기록 확인, hook errors: 2.
- [x] 전체 테스트 96개 통과.

### Phase 25 완료 기준
- [x] `admin_auth_middleware`: `ADMIN_TOKEN` 환경변수 기반 `/api/admin/*` 인증.
- [x] `CR6001`: Admin 인증 실패 (401).
- [x] `cors_middleware`: `CORS_ORIGINS` 환경변수 기반 CORS 정책.
- [x] Input Validation: `CJ1201` 에러 코드.
  - Board: title 1~200자, body 0~10000자
  - ERP: workflow_id 1~100자, supplier 1~200자, quantity 1~1000000
  - Blog: title 1~200자, category 1~50자
  - Shop: product_id 1~100자, quantity 1~1000000, price >= 0
- [x] Docker Compose: `ADMIN_TOKEN` + `CORS_ORIGINS` 환경변수 추가.
- [x] Docker 실 검증: 401/200, CORS headers, CJ1201 validation 전부 정상.
- [x] 전체 테스트 96개 통과.

### Phase 26 완료 기준
- [x] `L1Cache::warmup(module_ids)`: 기동 시 모듈 L1 사전 적재 (실패 시 기동 지속).
- [x] 기동 시 8개 모듈 warmup: base.board/ext.notice, base.erp/ext.erp_po, base.blog/ext.blog_tech, base.shop/ext.shop_electronics.
- [x] `GET /api/admin/status`: `database.l1_warm_modules` (L1 적재 모듈 수) 추가.
- [x] Docker 실 검증: `warmup: 8/8 modules`, `l1_warm_modules: 8`, 정상 요청 OK.
- [x] 전체 테스트 96개 통과.

### Phase 27 완료 기준
- [x] `GET /api/admin/export`: metrics + status + error logs 단일 JSON (attachment download).
- [x] `POST /api/admin/ping/trigger`: DB backend + Redis 상태 즉시 확인.
- [x] 대시보드 히트맵: 도메인별 요청 수 밝기로 표현.
- [x] 대시보드 서버 제어: Ping Now, Export JSON, Enable Alerts 버튼.
- [x] 브라우저 알림: hook errors 증가 시 Notification API.
- [x] Docker 실 검증: ping/trigger, export headers, 히트맵/제어 버튼 렌더링.
- [x] 전체 테스트 96개 통과.

### Phase 28 완료 기준
- [x] `docs/openapi.yaml` 갱신: 19개 endpoint + securitySchemes(bearerAuth/adminAuth) + CJ1201 422.
- [x] `docs/openapi.json`: YAML→JSON 변환 (서빙용).
- [x] `GET /openapi.json`: spec JSON 서빙.
- [x] `GET /swagger`: Swagger UI (swagger-ui-dist CDN).
- [x] Docker: docs/ 복사 (Dockerfile + .dockerignore).
- [x] Docker 실 검증: 19 endpoint, securitySchemes, /swagger 렌더링.
- [x] 전체 테스트 96개 통과.

### Phase 29 완료 기준
- [x] `tests/phase29_integration.rs`: 5개 E2E integration test.
  - D29-1: Board Create + DB 검증 (get_board_post)
  - D29-2: ERP Create + DB 검증 (get_erp_purchase_order)
  - D29-3: Input Validation (빈 title, 음수 quantity → CJ1201)
  - D29-4: Cache Invalidation cycle (적재→무효화→재로드)
- [x] `loadtest/k6.js`: 100→500 VUs 부하 테스트 스크립트.
- [x] `loadtest/README.md`: k6 설치/실행/해석 가이드.
- [x] 전체 테스트 101개 통과 (96 + 5 신규).

### Phase 30 완료 기준
- [x] `tests/phase30_chaos.rs`: 3개 chaos/fault-injection test.
  - T30-1: Redis(L2) 장애 → L3 fallback (FailingRedisStore)
  - T30-2: Recovery cycle (invalidate → 재로드)
  - T30-3: Save 장애 → 에러 응답 (패닉 아님, FailingStore)
- [x] Docker 실 검증: Redis 없는 환경에서 L1/L3 fallback 요청 성공.
- [x] 전체 테스트 104개 통과.

### Phase 31 완료 기준
- [x] `.github/workflows/ci.yml`: CI (Windows MSVC test + Linux clippy).
- [x] `.github/workflows/deploy.yml`: CD (Docker build + GHCR push).
- [x] `.env.example`: 환경변수 예제 (DB/Redis/JWT/rate-limit/admin/CORS).
- [x] YAML 구문 검증 OK.
- [x] 전체 테스트 104개 통과.

### Phase 32 완료 기준
- [x] `src/http.rs`: request_id 미들웨어 (X-Request-Id 헤더 + structured logging).
- [x] `monitoring/prometheus-alerts.yml`: SLO 기반 alert 7개 (HighErrorRate, HighLatency, CacheMiss, HookError, SandboxCrash/Timeout, DBDown).
- [x] `monitoring/grafana-dashboards/slo.json`: SLO dashboard (availability, error rate, latency, hook success + timeseries).
- [x] `prometheus.yml` + `docker-compose.yml`: alert rules + dashboard mount.
- [x] Docker 실 검증: X-Request-Id 헤더 + logging + Prometheus 7 alert 로드.
- [x] 전체 테스트 104개 통과.

### Phase 33 완료 기준
- [x] `src/main.rs`: graceful shutdown (SIGTERM/SIGINT + `with_graceful_shutdown`).
- [x] `src/main.rs`: config validation (DATABASE_BACKEND/REDIS_ENABLED/JWT_SECRET/RATE_LIMIT_PER_MIN).
- [x] `src/http.rs`: `/health/deep` (L3 DB 연결 + L1 cache + engine 상태).
- [x] Docker 실 검증: `/health/deep` ok, SIGTERM graceful shutdown, 잘못된 config → CR0004.
- [x] 전체 테스트 104개 통과.

### Phase 34 완료 기준
- [x] `src/engine/runtime.rs`: `Engine::warm_up()` (rquickjs JIT prewarm).
- [x] `src/meta/redis.rs`: `RedisLikeStore::ping()` (trait + RealRedisStore + InMemoryPubSub).
- [x] `src/cache.rs`: `L1Cache::redis_ping()` (redis_store 대필).
- [x] `src/http.rs`: `/health/deep`에 Redis PING 추가.
- [x] `src/main.rs`: 기동 시 engine warm_up 호출.
- [x] Docker 실 검증: warm-up log + Redis PING ok.
- [x] 전체 테스트 104개 통과.

### Phase 35 완료 기준
- [x] `src/metrics.rs`: `total_requests`/`total_errors` + SLO availability + error budget metric.
- [x] `src/http.rs`: `request_id_middleware`에 SLO 카운터 증가 (State-based).
- [x] `monitoring/grafana-dashboards/slo.json`: SLO availability + error budget panel.
- [x] Docker 실 검증: `/metrics`에 SLO metric 확인.
- [x] 전체 테스트 104개 통과.

### Phase 36 완료 기준
- [x] `migrations/003_tags_comments.sql`: board_tags + blog_comments 테이블.
- [x] `src/meta/store.rs`: BoardTag/BlogComment 모델 + DataStore trait (4 methods).
- [x] MemoryStore + PostgresStore 구현.
- [x] `src/http.rs`: `/api/tags` (POST+GET), `/api/blog/comments` (POST+GET).
- [x] `tests/phase29_integration.rs`: board_tag_crud + blog_comment_crud.
- [x] Docker 실 검증: migration + 태그/댓글 CRUD API.
- [x] 전체 테스트 106개 통과.

### Phase 37 완료 기준
- [x] `migrations/004_tenant.sql`: 6 테이블에 `tenant_id` 컬럼 + 인덱스.
- [x] `src/meta/store.rs`: 6 entity에 `tenant_id` 필드 + `default_tenant()` 헬퍼.
- [x] `DataStore` trait에 scoped 메서드 4개: `list_{board_posts,blog_articles,shop_orders,erp_orders}_for_tenant`.
- [x] `PostgresStore` + `MemoryStore` 구현 + INSERT/SELECT 쿼리 tenant_id 포함.
- [x] `src/http.rs`: `GET /api/tenant/posts?tenant={id}` 엔드포인트 (`X-Tenant-Id` 헤더 + query 지원).
- [x] `tests/phase29_integration.rs`: `tenant_isolation_scoped_list`.
- [x] Docker 실 검증: migration 적용 + `?tenant=default`(1 post) vs `?tenant=acme`(0 post) 격리 정상.
- [x] 전체 테스트 107개 통과.

### Phase 38 완료 기준
- [x] `migrations/005_shop_products.sql`: `shop_products`/`shop_carts` 스키마 (extra=TEXT, tenant 포함).
- [x] `src/meta/store.rs`: `ShopProduct`/`ShopCart` + DataStore 6메서드 (Memory+Postgres).
- [x] `src/http.rs`: 4 엔드포인트 (`/api/shop/products`, `/api/shop/carts`).
- [x] `tests/phase29_integration.rs`: `shop_product_and_cart_crud`.
- [x] **로컬** `cargo test` 108개 통과 (0 실패).
- [x] **Docker**: 상품/카트 CRUD 정상, price/stock/slug round-trip, panic 없음.
- [x] **Linux release 직렬화 특이점** 발견·우회 (안전한 패턴 문서화).
- [x] 핸드오버: `docs/setting/HANDOVER_phase_38_shop.md`.

### Phase 39 완료 기준
- [x] `src/rate_limit.rs`: `TokenBucket` (최대 N개/분, refill), `CircuitBreaker` (`Closed/Open/HalfOpen`), `CircuitState`.
- [x] `CircuitBreaker`가 `AppState`에 `Arc<RwLock<CircuitBreaker>>`로 통합.
- [x] `rate_limit_middleware`에 회로 차단기 연결: Open 상태면 `CR4004`로 429 거부, 5xx 응답 시 실패 기록, 2xx/4xx 시 성공 기록.
- [x] `/health` 엔드포인트에 `circuit_breaker` 상태 노출.
- [x] `tests/phase39_circuit_breaker.rs`: 9개 테스트 (TokenBucket + CircuitBreaker 동작).
- [x] 전체 테스트 117개 통과 (0 실패).
- [x] 핸드오버: `docs/setting/HANDOVER_phase_39_rate_limit.md`.

### Phase 40 완료 기준
- [x] `src/http.rs`: Admin Extension(모듈) 관리 5개 엔드포인트 — `GET /api/admin/modules` (목록+L1캐시), `GET /api/admin/modules/:id` (상세), `POST /api/admin/modules/:id/toggle` (활성화 토글), `POST /api/admin/modules/:id/reload` (L1 무효화), `POST /api/admin/modules/deploy` (컴파일 검증→저장→무효화).
- [x] `AppState::first_pipeline()`: board/domains 공통 store+cache+loader 접근 헬퍼.
- [x] `ModuleLoaderRef::compile_validate()`: 배포 전 스크립트 구문 검증 공개.
- [x] `DataStore::get_module_raw()`: 활성화 상태 무관 모듈 조회 (admin 토글용, PostgresStore 오버라이드).
- [x] `src/admin.rs`: 대시보드에 Extension Modules 섹션 (목록/토글/리로드/배포 UI).
- [x] `tests/phase40_admin_extension.rs`: 6개 테스트 (목록/토글/버전증분/캐시무효화/직렬화).
- [x] `docs/openapi.yaml` + `docs/openapi.json`: 5개 엔드포인트 문서화.
- [x] **로컬** `cargo test` 123개 통과 (0 실패).
- [x] **Docker 실 검증**: 목록(8개), 상세, 토글 off/on (get_module_raw 정상), reload, 유효/무효/필드누락 deploy, 미존재(404), 미인증(401) 모두 정상.
- [x] **SLO 벤치마크 (k6)**: `loadtest/k6_slo.js`(50 VU×30s, Rate Limiter 배제) 실행 — p95=79.6ms(SLO<500ms ✅), 에러률 0%(<5% ✅), 처리량 606 req/s(>500 ✅), 100% 체크 성공.
- [x] **SLO 동결**: `docs/setting/SLO.md`(목표/측정값/Rate Limiter 분리 기준/k6 threshold/회귀 기준).
- [x] `loadtest/README.md`: k6 Docker 실행 방법(`loadimpact/k6` 이미지, 동일 compose 네트워크) 문서화.
- [x] 핸드오버: `docs/setting/HANDOVER_phase_40_admin_ext.md`.

### Phase 40 SLO 벤치마크 완료 기준 (최종 마무리)
- [x] k6 부하 테스트 실측 (50 VU, 30s, Rate Limiter 배제).
- [x] SLO 목표 수치 동결 + 실측 근거 기록 (`docs/setting/SLO.md`).
- [x] Rate Limiter 과부하 시 429(CR4001) 정상 동작 확인 (별도 검증).
- [x] `docker-compose.yml`의 `RATE_LIMIT_PER_MIN` 측정용 일시 상승(60000) 후 원복(100).
- [x] 최종 회귀: 로컬 `cargo test` 123개 전부 통과.
- [x] SLO.md + HANDOVER + STATUS 갱신.

### Phase 41 Step 1 완료 기준 (계층형 템플릿 - 데이터 모델/채널/읽기 API)
- [x] `migrations/006_meta_templates.sql`: `meta_templates` 테이블 (parent_id/ttype/domain/payload TEXT/source/file_path/version/enabled) + `board_posts`/`blog_articles`/`shop_products`에 `template_id` 컬럼.
- [x] `src/meta/store.rs`: `MetaTemplate` 모델 + `TemplateTree`(재귀) + `DataStore` 5개 메서드 (`get_template`/`list_templates`/`list_template_children`/`upsert_template`/`build_template_tree`).
- [x] `MemoryStore` + `PostgresStore` 구현 (테넌트/도메인/부모 필터, 활성 버전 `DISTINCT ON`, Phase 38 직렬화 준수: payload=TEXT, COALESCE NULL-안정).
- [x] 순환 참조 방지: `build_tree_inner` (Box::pin) + `(template_id,version)` HashSet 방문 추적.
- [x] 파일 채널: `templates/*.json`(4개 샘플: site.shop/page.home/menu.main/body.blog_post) + `seed_templates_from_files()` (멱등, source='file').
- [x] `src/http.rs`: 읽기 API 3개 — `GET /api/templates`(목록, ?tenant=&domain=&parent=), `GET /api/templates/:id`(상세), `GET /api/templates/:id/tree`(트리 결합, `TemplateQuery`).
- [x] `tests/phase41_template.rs`: 6개 테스트 (upsert+버전증분/필터/트리결합/미존재/순환방지/파일시드멱등).
- [x] 로컬 `cargo test` 129개 통과 (0 실패).
- [x] 서버 기동 시 파일 시드 실동작 확인 (4개 템플릿 적재).
- [x] 설계: `docs/setting/PHASE41_DESIGN.md` (단1/단2 분할, Q1-2단1/단2 2단 분할, Q2-JSON파일 1개당 템플릿 1개, Q3-기존 도메인 테이블+template_id).

### Phase 41 Step 2 완료 기준 (계층형 템플릿 - 인스턴스 생성/개선 전파/사이트 프로비저닝)
- [x] `migrations/007_template_instance.sql`: `board_posts`/`blog_articles`/`shop_products`에 `template_version`(int4)/`sync_policy`(text) 컬럼 추가.
- [x] `src/meta/store.rs`: `BoardPost`/`BlogArticle`/`ShopProduct` 모델에 `template_id`/`template_version`/`sync_policy` 필드 추가 + `default_sync_policy()`("frozen").
- [x] `DataStore` 4개 신규 메서드: `apply_template_to_board_post`(바인딩 기록) / `bump_template_version`(활성 버전 +1) / `propagate_template_to_board_posts`(live 인스턴스 재적용) / `apply_site_template`(site body 노드 테넌트별 복사).
- [x] `MemoryStore` + `PostgresStore` 구현 (Phase 38 직렬화 준수: 정적 쿼리 분기, TEXT payload, i64/i32 네이티브 타입, now() 디폴트).
- [x] `src/http.rs`: Step 2 API 4개 — `POST /api/board/items?template_id=&sync_policy=`(인스턴스 생성, 템플릿 fields default 수집 + 파이프라인 생성 + 바인딩 기록), `POST /api/admin/templates/:id/bump`, `POST /api/admin/templates/:id/propagate`, `POST /api/admin/tenants/:id/apply-site-template?site_template_id=`.
- [x] `src/admin.rs`: 대시보드 "Templates (Hierarchical)" 섹션 (트리 뷰 + Bump/Propagate 버튼 + Site Provisioning).
- [x] `docs/openapi.yaml`/`.json`: 템플릿 엔드포인트 7개 문서화 (Step 1 3개 + Step 2 4개).
- [x] `tests/phase41_template.rs`: 4개 신규 테스트 (바인딩 기록/live-only 전파/버전 증분/사이트 프로비저닝).
- [x] 로컬 `cargo test` 133개 통과 (0 실패).
- [x] Windows MSVC 빌드 확인 완료.
- [ ] Docker 실 검증 (다음 세션 — Phase 38 직렬화 특이점이 Docker에만 발생하는지 확인 필요).

### Phase 42 P2 완료 기준 (Blog/Shop 스태킹 확장)
- [x] `src/meta/store.rs`: `DataStore` 5개 신규 메서드 — `apply_template_to_blog_article` / `apply_template_to_shop_product` / `propagate_template_to_blog_articles` / `propagate_template_to_shop_products` / `propagate_template_to_all_domains`.
- [x] `MemoryStore` + `PostgresStore` 구현 (Phase 38 직렬화 준수: 5파라미터, TEXT payload, i64/i32).
- [x] `src/http.rs`: `POST /api/blog/items?template_id=&sync_policy=` + `POST /api/shop/items?template_id=&sync_policy=` (템플릿 fields default 수집 + 도메인 파이프라인 생성 + 바인딩 기록).
- [x] `admin_template_propagate`에 `domain=all|board|blog|shop` 옵션 추가.
- [x] `src/admin.rs`: 대시보드 propagate에 domain 선택 dropdown (all/board/blog/shop).
- [x] `docs/openapi.yaml`/`.json`: 2개 신규 엔드포인트 + propagate domain 파라미터 문서화.
- [x] `tests/phase41_template.rs`: 5개 신규 테스트 (블로그/쇼핑몰 바인딩, live-only 전파×2, all-domains 전파).
- [x] `tests/phase30_chaos.rs`: FailingStore 5개 메서드 위임 추가.
- [x] 로컬 `cargo test` 138개 통과 (0 실패).
- [x] Windows MSVC 빌드 확인 완료.

### Phase 42 P3 완료 기준 (대시보드 인스턴스 생성 UI)
- [x] `src/admin.rs`: 인스턴스 생성 섹션 HTML 추가 (템플릿 dropdown, 도메인/sync_policy 선택, title 입력, 생성 버튼).
- [x] `createInstance()` JS 함수 (도메인별 API 호출: board→/api/board/items, blog→/api/blog/items, shop→/api/shop/items).
- [x] `renderTemplates()`에서 body 템플릿 dropdown 자동 갱신.
- [x] Windows MSVC 빌드 확인 완료.

### Phase 42 P4 완료 기준 (다중 도메인 체인 실서버 검증)
- [x] `templates/`: 4개 신규 샘플 템플릿 (body.hero, body.footer, menu.sub_products, menu.sub_categories).
- [x] `loadtest/phase42_scenario.bat`: 6단계 시나리오 스크립트 (헬스체크 → 템플릿 목록 → 트리 → 인스턴스 생성 → bump+propagate → 프로비저닝).
- [x] 실서버 검증: 8개 템플릿 시드, 트리 8개 노드, 인스턴스 생성 OK, bump OK, propagate OK, 프로비저닝 OK.
- [x] `apply_site_template` 소스 테넌트 "site" → "default" 수정 (파일 시드와 일치).
- [x] `tests/phase41_template.rs`: `apply_site_template_copies_body_nodes` 테스트 tenant "site" → "default" 수정.
- [x] 로컬 `cargo test` 138개 통과 (0 실패).

### Phase 42 P5 완료 기준 (Redis 캐시 연동)
- [x] `src/cache.rs`: `template_cache_get/set/invalidate/publish_invalidation` 4개 메서드 추가.
- [x] `src/http.rs`: `template_list`/`template_get`/`template_tree` 핸들러에 Redis 캐시 경로 추가 (REDIS_ENABLED=true 시).
- [x] `admin_template_bump`에 캐시 무효화 (detail + tree 키 삭제 + Pub/Sub 발행).
- [x] `REDIS_ENABLED=false` 시 기존 DB 직접 경로 (무영향).
- [x] 로컬 `cargo test` 138개 통과 (0 실패).

### Phase 42 P1 완료 기준 (Docker 실서버 검증)
- [x] WSL Ubuntu Docker 29.2.0 확인 (meta_server, PostgreSQL, Redis, Prometheus, Grafana 컨테이너 전부 실행).
- [x] Dockerfile에 `templates/` 디렉토리 COPY 추가.
- [x] docker-compose에 003-007 마이그레이션 마운트 추가.
- [x] `collect_template_fields` 중첩 payload 구조(payload.payload.fields) 파싱 수정.
- [x] blog_instance_create에 category 필드 추가.
- [x] 실서버 검증: 8개 템플릿 시드, blog/shop/board 인스턴스 생성 OK, bump/propagate OK, site provisioning OK.
- [x] 로컬 `cargo test` 138개 통과 (0 실패).

### 기능별 테이블 생성 스크립트 (`scripts/schemas/`)
- [x] `scripts/schemas/board.sql`: board_posts, board_tags.
- [x] `scripts/schemas/blog.sql`: blog_articles, blog_comments.
- [x] `scripts/schemas/shop.sql`: shop_products, shop_carts, shop_orders.
- [x] `scripts/schemas/meta.sql`: meta_modules, meta_templates.
- [x] `scripts/schemas/erp.sql`: erp_purchase_orders.

---

## 4. 진행 지침 (Handover Instructions)

> 다음 개발자(또는 다음 세션의 AI 에이전트)가 이 문서를 읽고 바로 작업을 이어가도록 기록합니다.

### 현재 진행을 위한 핵심 사항
1. **빌드 환경:** `source /c/temp/load_msenv.sh` + `export RUSTFLAGS='-C link-arg=/LIBPATH:C:\wsdk10\10.0.26100.0\um\x64 -C link-arg=/LIBPATH:C:\wsdk10\10.0.26100.0\ucrt\x64'` (상세: docs/setting/BUILD_ENV_SETUP.md)
2. **의존성 버전 고정:** `Cargo.toml`에서 rquickjs가 `0.14.0`(features: classes, futures, parallel), redis가 `0.27.6`(features: tokio-comp, connection-manager).
3. **에러 코드 체계:** `[도메인 1자][R|J][종류 1자][개별번호 3자]` (6자리). CJ1104(일반 JS throw)는 Phase 1에서 추가.
4. **패닉 금지:** `src/engine`, `src/sdk`에서 `unwrap()`, `expect()`, `panic!` 사용 절대 금지.
5. **중요 한계:** rquickjs 0.14에서 interrupt handler는 Runtime당 1개. ModuleLoader는 `compile_engine`(cpu_timeout_ms=0)을 별도 사용.
6. **Redis/PostgreSQL 설정:** 기본값 — `DATABASE_BACKEND=postgres`, `REDIS_ENABLED=false`. Redis/PG 인스턴스 없으면 자동 InMemoryPubSub/MemoryStore 폴백.
7. **환경변수:** `DATABASE_BACKEND`, `DATABASE_URL`, `REDIS_ENABLED`, `REDIS_URL` (상세: HANDOVER_phase_4_redis_integration.md)

### 단절 시 이어서 시작하는 방법
1. `[WIP 체크포인트]`의 "다음 액션"을 읽는다.
2. "중단 위치"에 명시된 파일과 함수를 열어서 바로 그 위치부터 수정한다.
3. "주의사항"의 내용을 확인하고 그 제약 조건을 지키면서 작업을 진행한다.
4. 작업을 완료하면 이 문서의 `Phase별 진행 상태`를 갱신하고 `WIP 체크포인트`를 정리한다.
