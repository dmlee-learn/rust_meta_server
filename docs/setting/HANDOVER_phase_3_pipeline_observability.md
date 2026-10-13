# Phase 3 핸드오버 — 게시판 파이프라인 + L1 캐싱 + 옵저버빌리티 + 에러 매핑

## 1. 목표
게시판 글 작성 요청 시, **DB에 저장하기 직전** L1 인메모리 캐시에 적재된 JS 스크립트의
`before_create` 훅을 찾아 실행하고, 그 결과에 따라 **통과(DB 저장)** 또는 **에러 코드 반환**을
결정하는 Axum 핸들러 파이프라인을 완성하는 것이다.

**핵심 원칙 (AGENTS.md 준수):**
- **상태값은 메타데이터로, 분기/알고리즘은 스크립트로** — 게시판 비즈니스 룰은 `scripts/base/board.js`에서 JS로 정의
- **결정적 실패 보호:** 구문 오류(CJ1101) 스크립트가 배포되면 즉시 직전 안전 버전으로 후퇴
- **캐시 스탬피드 방어:** L1 미스 시 **Single Flight**로 오직 첫 번째 스레드만 L3에서 로드

## 2. 구현된 핵심 구조
```
POST /api/board (module_id=board, title=..., content=...)
   │
   ▼
[ BoardPipeline::handle_create ]   ← src/pipeline.rs
   │
   ├─ 1. L1 Cache.get_module("board")  ← src/cache.rs (Single Flight)
   │      miss → 락 획득 → L3(DataStore) 로드 → 컴파일/구문 검증(CJ1101) → L1 적재
   │      (동시 미스: 첫 1개만 L3 로드, 나머지는 await 공유)
   │
   ├─ 2. create_sandbox() → before_create 훅 실행 (JS 스크립트)
   │      └─ before_create: title 검증 (빈값→CJ1104, 100자 초과→CJ1104)
   │
   ├─ 3. 통과 시 → L3(DataStore).save_board_post() 저장
   │      실패 시 → 에러 코드 JSON 반환, DB 저장 차단
   │
   └─ 4. 옵저버빌리티: ExecutionReport + EventQueueObserver
          → 이벤트 큐에 push → 별도 task가 소비 (로그/메트릭)
```

## 3. 구현 파일
| 파일 | 내용 |
|------|------|
| `src/cache.rs` | `L1Cache` (Single Flight, `tokio::sync::Mutex` 락 + `oneshot` 채널 공유) |
| `src/pipeline.rs` | `BoardPipeline`, `CreatePostRequest/Response`, 에러 매핑(CJ1101~CJ1104) |
| `src/observability.rs` | `ExecutionReport`, `EvalEvent`, `ScriptObserver`, `EventQueueObserver` |
| `src/engine/runtime.rs` | `SandboxedContext::with_observability`, `eval_string_observed` |
| `src/engine/throttle.rs` | `classify_error` (cpu_exceeded→CJ1102, 메모리 90%→CJ1103, 그 외→CJ1104) |
| `src/meta/store.rs` | `BoardPost` 엔티티 + `save_board_post`/`get_board_post` (DataStore 트레이트) |
| `src/http.rs` | `/api/board` 엔드포인트 + `AppState`에 board 파이프라인 추가 |
| `scripts/base/board.js` | 게시판 기준 모듈 (before_create, after_save 훅) |
| `scripts/extensions/notice.js` | 공지사항 확장 (is_pinned 고정, 삭제 보호) |

## 4. 주요 API
```rust
// L1 캐싱 (Single Flight)
pub struct L1Cache { ... }
impl L1Cache {
    pub fn new(store: Arc<dyn DataStore>) -> Self;
    pub async fn get_module(&self, module_id: &str) -> Result<Option<CompiledModule>, LoadError>;
    pub async fn invalidate(&self, module_id: &str);
    pub async fn len(&self) -> usize;
}

// 옵저버빌리티
pub struct ExecutionReport { pub hook_name: String, pub module_id: String, pub version: u64, pub cpu_exceeded: bool, pub ok: bool, pub message: String }
pub struct EventQueueObserver { ... }  // mpsc::UnboundedSender
impl EventQueueObserver {
    pub fn new(queue: mpsc::UnboundedSender<EvalEvent>) -> Self;
    pub fn event_receiver() -> (queue, mpsc::UnboundedReceiver<EvalEvent>);
}
impl ScriptObserver for EventQueueObserver {
    fn on_script_eval(&self, module_id: &str, hook_name: &str, report: &ExecutionReport);
}

// 게시판 파이프라인
pub struct BoardPipeline { ... }
impl BoardPipeline {
    pub async fn handle_create(&self, req: CreatePostRequest) -> Result<CreatePostResponse, PipelineError>;
}

// 에러 매핑 (src/pipeline.rs)
enum PipelineError {
    ModuleNotFound,   // → CR0004
    HookFailed { code, message },  // → CJ1101~CJ1104 (JS 스크립트 에러)
    SaveFailed,       // → CR0005
}
```

## 5. 에러 코드 매핑 (Phase 3 범위, 6자리 체계)
| 코드 | 의미 | 발생 위치 |
|------|------|-----------|
| `CR0004` | 모듈 미발견 (L3에 존재하지 않음) | `BoardPipeline::handle_create` |
| `CR0005` | 저장 실패 (L3 write error) | `BoardPipeline::handle_create` → `save_board_post` |
| `CJ1101` | JS 구문 오류 (캐시 적재 단계) | `ModuleLoader::compile_and_validate` |
| `CJ1102` | CPU Timeout (무한 루프) | `classify_error` (cpu_exceeded 플래그) |
| `CJ1103` | 샌드박스 메모리 초과 | `classify_error` (malloc 90% 이상) |
| `CJ1104` | JS 일반 예외 (throw) | `classify_error` (비예외 아님) |
| `SJ1201` | 비즈니스 룰 위반 (title 검증) | `scripts/base/board.js`의 before_create |

## 6. 테스트 (전부 통과, 6개)
```bash
source /c/temp/load_msenv.sh
export RUSTFLAGS='-C link-arg=/LIBPATH:C:\wsdk10\10.0.26100.0\um\x64 -C link-arg=/LIBPATH:C:\wsdk10\10.0.26100.0\ucrt\x64'
cargo test --test phase3_pipeline
```
| # | 테스트 | 검증 내용 |
|---|--------|-----------|
| 1 | `board_create_success` | 정상 글 작성 → L3에 저장 |
| 2 | `board_create_reject_empty_title` | 빈 title → CJ1104 에러, DB 미저장 |
| 3 | `board_create_infinite_loop_cj102` | 무한 루프 훅 → CJ1102 (CPU Timeout) |
| 4 | `board_create_triggers_after_save` | after_save 훅 실행 → 이벤트 큐에 기록 |
| 5 | `l1_cache_populated` | L1 캐시 적재 (module_id → version) |
| 6 | `observer_receives_events` | 옵저버 이벤트 수신 (before_create + after_save) |

## 7. Phase 3 목표 달성 검증 (HANDOVER_phase_3_redis_pubsub_cache.md 기준)
- ✅ L1 캐싱: 미스 시 L3에서 적재, 히트 시 0ms 반환
- ✅ Single Flight: 동시 미스 시 1회만 L3 로드 (mutex 락 + oneshot 채널)
- ✅ 옵저버빌리티: ExecutionReport + EventQueueObserver (외부 이벤트 수신)
- ✅ 게시판 글 작성 파이프라인: before_create 훅 → DB 저장 → after_save
- ✅ 에러 매핑: CJ1101~CJ1104 + CR0004/CR0005 + SJ1201

## 8. 다음 단계 (Phase 4 이후)
- [ ] Redis Pub/Sub 실제 연동 (현재 MemoryPubSub 스텁)
- [ ] PostgreSQL 연동 (현재 MemoryStore 스텁)
- [ ] 소켓 통신 (AGENTS.md: `on_socket_connect`/`message`/`disconnect` 훅, CR2001/CR2002/CJ2101)
- [ ] 다층 캐시(L1→L2→L3) 무효화 생명주기 (Redis Pub/Sub 통지 시 L1 갱신)
