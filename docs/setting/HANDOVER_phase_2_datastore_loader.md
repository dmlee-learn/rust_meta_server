# Phase 2 핸드오버 — DataStore + 모듈 로딩 파이프라인 + CJ1101 + Redis 스텁

> **작성일:** 2026-09-30
> **다음 세션의 개발자(AI 에이전트)에게**: 이 문서를 읽으면 Phase 2의
> 구현 내용을 그대로 이해하고, Phase 3 (L1 캐시 + 훅 발동 + 핸들러 파이프라인)
> 로 바로 이어서 작업할 수 있습니다.
>
> **이전 세션의 작업이 이 문서와 충돌할 경우 이 문서가 우선합니다.**
> **이 문서를 읽고 나서야 코드 베이스를 탐색해야 합니다.**

---

## 0. Phase 2 결정 사항 (사용자와 합의)
| 항목 | 결정 | 근거 |
|------|------|------|
| DB 백엔드 | **PostgreSQL 기본**, 메모리 스텁으로 폴백 | Q1-1 |
| DB 드라이버 | `tokio-postgres` 0.7.18 | Q2-2 |
| Redis | **기능 구현만, 기본 비활성화** (`REDIS_ENABLED=false`) | Q1-1 |
| 실데이터 | **인스턴스 없이 메모리 스텁으로 진행** | Q2-3 |
| 범위 | DataStore + 모듈 로딩 + CJ1101 + Redis 스텁 (한 번에 완료) | Q3-1 |

---

## 1. 이 Phase에서 완료한 것 (Done)

### 1.1 신규 파일
```
src/
├── meta/
│   ├── mod.rs                  # pub mod store; pub mod redis; re-export
│   ├── store.rs                # DataStore 트레잇 + PostgresStore + MemoryStore + MetaModule
│   └── redis.rs                # RedisLikeStore 트레잇 + InMemoryPubSub 스텁 (비활성 기본)
├── engine/
│   └── loader.rs               # ModuleLoader (Base→Extension 로딩 + CJ1101 감지)
scripts/
├── base/board.js               # 기준 게시판 모듈 (before_create, after_save, before_delete)
└── extensions/notice.js        # 확장 공지사항 모듈 (Base 상속 + pinned 검증)
tests/
└── phase2_loader.rs            # Phase 2 통합 테스트 10개
```

### 1.2 핵심 구현
| 항목 | 위치 | 내용 |
|------|------|------|
| `DataStore` | `src/meta/store.rs` | `async_trait`: `get_module`, `list_enabled_modules`, `upsert_module` |
| `MetaModule` | `src/meta/store.rs` | module_id, kind, base_module_id, script_source, content_hash, version(i32), enabled |
| `PostgresStore` | `src/meta/store.rs` | `tokio_postgres::Config::parse().connect(NoTls)` — 인스턴스 없으면 `DbConnect` 에러 |
| `MemoryStore` | `src/meta/store.rs` | `RwLock<HashMap<String, MetaModule>>` 스텁 + `seed()` |
| `create_data_store` | `src/meta/store.rs` | `DATABASE_BACKEND=postgres|memory` 팩토리 |
| `RedisLikeStore` | `src/meta/redis.rs` | `get_cache`, `set_cache`, `publish_invalidation`, `subscribe_invalidation` |
| `InMemoryPubSub` | `src/meta/redis.rs` | `broadcast::channel` 기반 in-process Pub/Sub 스텁 |
| `create_redis_store` | `src/meta/redis.rs` | `enabled==false` 시 `Err(Disabled)` (기본) |
| `ModuleLoader` | `src/engine/loader.rs` | `load_modules`(L3 적재+CJ1101 검증), `bind_extension`(Base→Extension 바인딩), `compile_and_validate`(CJ1101) |
| `LoadError` | `src/engine/loader.rs` | Syntax(CJ1101), Engine, CpuTimeout(CJ1102), Memory(CJ1103) |
| `HOOK_NAMES` | `src/engine/loader.rs` | before_request, after_save, before_create, before_update, before_delete, after_request |

### 1.3 Base→Extension 바인딩 방식 (중요)
`bind_extension`은 Base와 Extension 스크립트를 하나의 IIFE 템플릿으로 조합해 실행한다:

```js
var Base = (function(){ <base_script>; return api; })();
globalThis.Base = Base;
var Extension = (function(){
    var api = Object.create(Base);          // 프로토타입 = Base
    (function(){
        var own = {};
        <ext_script>;                         // ext는 own에 훅 정의
        for (var k in own) if (own.hasOwnProperty(k)) api[k] = own[k];
    })();
    return api;
})();
globalThis.Extension = Extension;
```

**발견한 함정**: 템플릿에서 `var api = {};`를 선언하면 base/ext 스크립트가 **또다시**
`var api={};`를 선언해 **변수 충돌**이 생기고, `Object.create(Base)`로 만든 api가
빈 `{}`로 덮어쓰여 **프로토타입 체이닝이 사라진다** (동등 객체가 프로토타입이 아님).
→ 템플릿의 `var api`를 제거하고, ext 훅을 `own` 객체에 모아 `hasOwnProperty`로
소유된 훅만 `api`에 복사하는 방식으로 해결했다. (테스트 `extension_inherits_base_hooks`로 검증)

### 1.4 의존성 추가
```
tokio-postgres = "0.7.18"
async-trait = "0.1.92"
thiserror = "2.0.21"
```
(`postgres-protocol`은 transitive로 추가됨)

---

## 2. 테스트 결과 (Total)

| 유형 | 파일 | 수 | 결과 |
|------|------|----|------|
| 유닛 (Phase 1) | `src/engine/runtime.rs` | 5 | ✅ |
| 기능 HTTP (Phase 1) | `tests/functional_http.rs` | 5 | ✅ |
| 통합 (Phase 1) | `tests/integration_engine.rs` | 7 | ✅ |
| **Phase 2** | `tests/phase2_loader.rs` | **10** | ✅ |
| **합계** | | **27** | **27/27 통과** |

### Phase 2 테스트 시나리오 (tests/phase2_loader.rs)
1. `memory_store_factory` — memory backend 팩토리
2. `postgres_store_factory_connect_fails_without_instance` — 인스턴스 없으면 연결 실패 (DbConnect)
3. `memory_store_upsert_and_get` — 스텁 저장/조회
4. `syntax_error_detected_as_cj1101` — **CJ1101 구문오류 감지**
5. `valid_base_ext_loads` — 정상 Base+Extension 로드 (2개)
6. `extension_inherits_base_hooks` — **Base→Extension 프로토타입 상속**
7. `base_hook_business_error_propagates` — Base 훅의 비즈니스 에러(SJ1201) 전파
8. `redis_store_disabled_by_default` — Redis 기본 비활성 (Disabled)
9. `redis_store_cache_read_write` — Redis 스텁 캐시 읽기/쓰기
10. `redis_pubsub_publish_subscribe` — Redis 스텁 Pub/Sub 발신/수신

### 실행 명령
```bash
source /c/temp/load_msenv.sh
export RUSTFLAGS='-C link-arg=/LIBPATH:C:\wsdk10\10.0.26100.0\um\x64 -C link-arg=/LIBPATH:C:\wsdk10\10.0.26100.0\ucrt\x64'
cargo test                      # 전체 27개
cargo test --test phase2_loader # Phase 2만
```

---

## 3. 에러 코드 체계 (Phase 2 추가)

| 코드 | 의미 | 발생 위치 |
|------|------|-----------|
| `CJ1101` | JS 스크립트 구문 오류 (캐시 적재 단계 차단) | `ModuleLoader::compile_and_validate` → `LoadError::Syntax` |

**AGENTS.md와의 대응**: CJ101은 Phase 2에서 `LoadError::Syntax`로 도입.
`LoadError`는 `InfraError`와 별도로, `Display`에서 `"CJ1101: ..."` 형태로 코드를 노출한다.
Phase 3에서 `LoadError` → `InfraError`/HTTP 응답 매핑을 통합할 수 있다.

기존 코드(CR0001/CR0002/CJ1102/CJ1103/CJ1104)는 Phase 1에서 그대로 유지.

---

## 4. 현재 상황과 주의사항

### 4.1 PostgreSQL 스키마
- `meta_modules` 테이블 스키마는 **별도 마이그레이션**이 필요하다.
  쿼리는 `module_id, kind, base_module_id, script_source, content_hash, version, enabled` 컬럼을 전제한다.
  Phase 3에서 인스턴스가 준비되면 스키마 DDL을 작성해야 한다.
- `PostgresStore::ensure_schema`는 현재 no-op (스키마 자동 생성 미구현).

### 4.2 Redis 스텁의 한계
- `InMemoryPubSub`은 **in-process** `broadcast` 채널만 지원. 실제 Redis 인스턴스와
  서버 간(멀티 프로세스) Pub/Sub은 **미구현**.
- `redis` crate 미도입 — 실제 Redis 연동은 활성화 시점에 추가.
- 기본 `RedisConfig.enabled = false` (Q1-1). 활성화하려면 설정에서 `enabled: true`로 변경.

### 4.3 모듈 로딩과 서버 기동
- 현재 `main.rs`는 **엔진 + HTTP 엔드포인트만** 기동한다 (Phase 1).
- `ModuleLoader`와 `DataStore`는 **테스트에서만 사용** 중이며, 서버 기동 시
  `scripts/` 로딩은 Phase 3의 핸들러 파이프라인에서 연결 예정이다.
- `scripts/base/board.js`, `scripts/extensions/notice.js`는 **샘플**이며,
  실제 메타데이터는 L3(PostgreSQL/메모리 스텁)에 저장되는 `MetaModule` 레코드로 관리된다.

### 4.4 `version` 타입
- `MetaModule.version`은 `i32` (PostgreSQL `int` 매핑). `u64`는 `ToSql` 구현이 없으므로 피함.
- `CacheEntry.version`은 `u64` (Redis, DB 미사용).

---

## 5. 다음 Phase(Phase 3)로 넘어가기 전 확인사항

- [x] `cargo build` 성공 (MSVC + RUSTFLAGS 설정된 shell에서)
- [x] `cargo test` 27/27 통과
- [x] `cargo clippy` 에러 없음
- [x] `cargo run` 후 `/health`가 `ok` 반환 (포트 3999 실측)
- [ ] (Phase 3) L1 인메모리 캐시 (Single Flight)
- [ ] (Phase 3) `before_request`/`after_save` 등 훅을 실제 HTTP 핸들러에서 발동
- [ ] (Phase 3) `LoadError` → `InfraError`/HTTP 응답 매핑 통합
- [ ] (Phase 3) PostgreSQL 스키마 DDL + 인스턴스 연동
- [ ] (Phase 3) Redis 실연동 (활성화 시)

---

## 6. 참고 파일
- `docs/DEVELOPMENT_PLAN.md` — 전체 개발 계획 (Phase 0~5)
- `docs/STATUS.md` — 진행 상태 (Phase 1 완료, Phase 2 완료 반영 필요)
- `docs/setting/HANDOVER_phase_1_engine_throttling.md` — Phase 1 핸드오버
- `src/meta/store.rs` — DataStore + PostgreSQL + 메모리 스텁
- `src/meta/redis.rs` — Redis 스텁 (비활성 기본)
- `src/engine/loader.rs` — Base→Extension 모듈 로딩 + CJ1101
- `scripts/base/board.js`, `scripts/extensions/notice.js` — 샘플 모듈
- `tests/phase2_loader.rs` — Phase 2 테스트
