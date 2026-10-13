# Phase 1 핸드오버 — rquickjs 엔진 초기화 + Throttling

> **작성일:** 2026-09-30
> **다음 세션의 개발자(AI 에이전트)에게**: 이 문서를 읽으면 Phase 1의
> 구현 내용을 그대로 이해하고, Phase 2 (Base/Extension 모듈 로딩)로
> 바로 이어서 작업할 수 있습니다.
>
> **이전 세션의 작업이 이 문서와 충돌할 경우 이 문서가 우선입니다.**
> **이 문서를 읽고 나서야 코드 베이스를 탐색해야 합니다.**

---

## 1. 이 Phase에서 완료한 것 (Done)

### 1.1 빌드 환경 (docs/setting/BUILD_ENV_SETUP.md에 상세 기록)
- Rust 1.94.0 + Visual Studio Build Tools 2022 (MSVC v14.44.35207) 설치
- MSYS2에서 MSVC 환경 로드: `source /c/temp/load_msenv.sh` (cmake 3.31.9 포함)
- Windows SDK lib 경로 문제 해결: `C:\wsdk10` junction으로
  `C:\Program Files (x86)\Windows Kits\10\`를 링크
- `.cargo/config.toml`: `linker = "link"`, `CC = <cl.exe 경로>`
- **중요**: `RUSTFLAGS`는 `.cargo/config.toml`에서 link-arg로 변환되지 않으므로
  shell에서 `export RUSTFLAGS='-C link-arg=/LIBPATH:C:\wsdk10\10.0.26100.0\um\x64 -C link-arg=/LIBPATH:C:\wsdk10\10.0.26100.0\ucrt\x64'` 설정 필요
- 전체 빌드 테스트 통과: rquickjs → QuickJS C 컴파일 → Windows SDK lib 링크 성공

### 1.2 코드 구조 (lib + bin 분리)
```
meta_server/
├── Cargo.toml            # rquickjs(0.14, features: classes,futures,parallel), axum, tokio, serde_json, tower
├── src/
│   ├── lib.rs            # pub mod core; pub mod engine; pub mod http
│   ├── main.rs           # 서버 기동: Engine::new() + build_router() + axum::serve
│   ├── core/
│   │   ├── mod.rs
│   │   └── error.rs      # InfraError enum (CR0001, CR0002, CJ1102, CJ1103, CJ1104)
│   ├── engine/
│   │   ├── mod.rs        # pub mod runtime; pub mod throttle; re-export
│   │   ├── runtime.rs    # Engine, SandboxedContext (Phase 1 핵심)
│   │   └── throttle.rs   # ThrottleConfig, CpuExceeded, classify_error
│   └── http.rs           # AppState, build_router, /health, /api/engine/info, /api/engine/eval
├── tests/
│   ├── integration_engine.rs   # 7개 통합 테스트
│   └── functional_http.rs      # 5개 기능 테스트 (tower::Service in-process)
├── docs/
│   ├── DEVELOPMENT_PLAN.md
│   ├── STATUS.md
│   ├── rquickjs/API_CHANGES.md
│   └── setting/
│       ├── BUILD_ENV_SETUP.md
│       ├── handover_template.md
│       ├── HANDOVER_phase_1_design.md
│       ├── HANDOVER_phase_1_env_setup.md
│       └── HANDOVER_phase_1_engine_throttling.md   # 이 문서
└── .cargo/config.toml
```

### 1.3 핵심 구현
| 항목 | 위치 | 내용 |
|------|------|------|
| `Engine` | `src/engine/runtime.rs` | `Runtime: Arc<Runtime>` + `ThrottleConfig`. `new()`에서 `Runtime::new()` + `set_memory_limit` + `set_max_stack_size` + `set_gc_threshold` |
| `SandboxedContext` | `src/engine/runtime.rs` | `Context: Context` + `cpu_exceeded: Arc<CpuExceeded>`. `create_sandbox()`에서 `Context::full(rt)` 생성 + CPU Timeout interrupt handler 적용 |
| `eval_i64` | `src/engine/runtime.rs` | JS 실행. 실패 시 `classify_error`로 CJ1102/CJ1103/CJ1104 분류 |
| `classify_error` | `src/engine/throttle.rs` | CPU 초과 플래그→CJ1102, 메모리 90% 이상→CJ1103, 일반 예외→CJ1104, 비예외→CR0002 |
| `ThrottleConfig` | `src/engine/throttle.rs` | `memory_limit`(기본 8MB), `max_stack`(256KB), `cpu_timeout_ms`(1초) |
| `CpuExceeded` | `src/engine/throttle.rs` | `AtomicBool` 플래그. interrupt handler가 초과 시 `mark()` |
| `InfraError` | `src/core/error.rs` | CR0001/CR0002/CJ1102/CJ1103/CJ1104. `code()`(6자리), `message()`, `Display`, `Debug` |
| HTTP 엔드포인트 | `src/http.rs` | `/health`, `/api/engine/info`(Throttle 설정+메모리 사용량), `/api/engine/eval`(JS 실행) |

### 1.4 rquickjs 0.14 API 변경 사항 (docs/rquickjs/API_CHANGES.md)
- `Runtime::new()`는 `Result<Runtime>` 반환 (0.7 이전과는 다름)
- `Context::full(&rt)` (0.14, base/custom/full/builder 제공)
- `Runtime::set_memory_limit(usize)` — 바이트 단위 메모리 한도
- `Runtime::set_max_stack_size(usize)` — 스택 한도
- `Runtime::set_interrupt_handler(Option<Box<dyn FnMut() -> bool + Send + Sync>>)` — CPU Timeout 구현 (true 반환 시 인터럽트)
- `Runtime::memory_usage()` → `qjs::JSMemoryUsage` (malloc_limit, malloc_size, memory_used_size 등)
- `Runtime::run_gc()` — 수동 GC
- `Context::with(|ctx| ...)` — 컨텍스트 스코프
- `ctx.eval::<V, _>(source)` — 스크립트 실행
- **주의**: `Rc` 기반이라 `Send`가 아니므로 `parallel` feature 필수 (Axum `State`가 `Send+Sync` 요구)

---

## 2. 테스트 결과 (Total)

| 유형 | 파일 | 수 | 결과 |
|------|------|----|------|
| 유닛 테스트 | `src/engine/runtime.rs` | 5 | ✅ 전부 통과 |
| 기능 테스트 (HTTP) | `tests/functional_http.rs` | 5 | ✅ 전부 통과 |
| 통합 테스트 | `tests/integration_engine.rs` | 7 | ✅ 전부 통과 |
| **합계** | | **17** | **17/17 통과** |

### 실행 명령
```bash
source /c/temp/load_msenv.sh
export RUSTFLAGS='-C link-arg=/LIBPATH:C:\wsdk10\10.0.26100.0\um\x64 -C link-arg=/LIBPATH:C:\wsdk10\10.0.26100.0\ucrt\x64'
cargo test          # 전체 테스트
cargo test --lib    # 유닛만
cargo run           # 서버 기동 (포트 3000)
```

### 테스트 시나리오
- **유닛**: 엔진 생성, 기본값, 정상 eval, CPU Timeout(CJ1102), 메모리 한도(CJ1103), JS throw(CJ1104)
- **기능**: `/health` 200+ok, `/api/engine/info` Throttle 설정 노출, 정상 eval 결과, 무한 루프→CJ1102, JS throw→6자리 에러코드
- **통합**: ThrottleConfig 기본값, 메모리 한도 런타임 반영, 정상 스크립트, 무한 루프→CJ1102(시간 검증), 메모리 OOM→CJ1103, GC 후 런타임 생존, 에러코드 6자리 명명 규칙

---

## 3. 서버 기동 검증 (실제 HTTP)

포트 3000이 다른 서비스(Express)에 점유되어 있으므로 **임시 포트 3999**로 검증:

```
[MetaServer] rquickjs engine initialized (memory_limit=8388608 bytes, cpu_timeout_ms=1000, malloc_limit=8388608 malloc_size=28616)
[MetaServer] listening on http://0.0.0.0:3999

GET  /health           → ok
GET  /api/engine/info  → {"code":"OK","cpu_timeout_ms":1000,"memory_limit":8388608,...}
POST /api/engine/eval  {"script":"6*7"}          → {"code":"OK","result":42}
POST /api/engine/eval  {"script":"무한루프"}      → {"code":"CJ1102","message":"스크립트 실행 시간 초과 (CPU Timeout)"}
POST /api/engine/eval  {"script":"1+1"} (CJ1102 후) → {"code":"OK","result":2}   # 서버 생존 확인
POST /api/engine/eval  {"script":"throw..."}     → {"code":"CJ1104","message":"JS 스크립트 실행 예외 (일반 throw)"}
```

**Phase 1 목표 달성**: 서버 기동 → rquickjs 런타임 초기화 → 메모리 제한(malloc_limit=8MB 반영) + CPU Timeout(CJ1102 발동) 강제 적용 확인.

---

## 4. 에러 코드 체계 (Phase 1 범위)

| 코드 | 의미 | 발생 위치 |
|------|------|-----------|
| `CR0001` | 시스템 메모리 부족 / 스레드 할당 실패 | (미사용, 예약) |
| `CR0002` | Runtime/Context 생성 실패 | `Engine::new()`, `create_sandbox()` |
| `CJ1102` | CPU Timeout (무한 루프) | interrupt handler 초과 |
| `CJ1103` | 샌드박스 메모리 한도 초과 | malloc_size >= 90% * limit |
| `CJ1104` | JS 스크립트 일반 예외 (throw) | CPU/메모리 초과 아님 |

에러는 항상 `{"code": "<6자리>", "message": "<문자열>"}` JSON 형태로 반환.

**명명 규칙**: `[도메인 C][레이어 R|J][종류 1자][개별번호 3자]` (6자리). CJ1104는 Phase 1에서 일반 JS throw를 별도로 구분하기 위해 추가 (AGENTS.md에는 CJ1101~CJ1103만 정의, CJ1101=구문오류는 Phase 2에 도입).

---

## 5. 현재 상황과 주의사항

### 5.1 rquickjs 0.14 OOM/CPU 구분의 한계
- rquickjs 0.14는 OOM과 CPU Timeout이 모두 `Error::Exception`으로 표출됨
- **구분 방식**:
  - CPU Timeout: interrupt handler가 `CpuExceeded` 플래그 설정 → CJ1102
  - 메모리 초과: `ctx.runtime().memory_usage()`에서 `malloc_size >= 90% * malloc_limit` → CJ1103
  - 그 외 예외: CJ1104
- OOM 발생 직후 `malloc_size`가 한도보다 약간 아래(261256 vs 262144)로 보고되므로
  **90% 임계값**을 사용 (실측 기반 결정)
- 이 방식은 "메모리 압력 상태"를 추정하는 것이지, 정확히 OOM을 포착하는 것은 아님.
  Phase 2에서 더 정교한 감지가 필요하면 QuickJS의 `js_throw`를 커스텀 hook으로 감지하는 방식으로 개선 가능

### 5.2 공유 Runtime 구조의 특성
- `Engine`은 1개 Runtime을 `Arc`로 공유. `SandboxedContext`는 요청별로 `Context::full` 생성
- interrupt handler는 Runtime 수준이라 **모든 Context에 공통 적용**됨
- 한 요청의 CPU 초과 플래그는 다른 요청에 영향 주지 않음 (각 `SandboxedContext`가
  독립 `CpuExceeded`를 소유)
- **주의**: interrupt handler는 Runtime에 1개만 존재. 동시 다중 요청 시 모든 요청이
  동일한 `start` 시각 기준의 1개 handler를 공유하게 됨 → **현재는 요청별 독립된 handler가
  필요하나, rquickjs가 Runtime당 1개 handler만 허용하므로 Phase 2에서 컨텍스트별
  실행 시간 추적 또는 별도 Runtime pooling로 해결 필요**

### 5.3 lib/binary 분리의 이유
- `tests/`는 통합 테스트로 `meta_server` 라이브러리를 직접 import
- `src/lib.rs`가 `core`/`engine`/`http` 모듈을 노출, `src/main.rs`는 기동 로직만
- Phase 2의 Base/Extension 모듈 로딩도 라이브러리 모듈로 추가 예정

---

## 6. 다음 Phase(Phase 2)로 넘어가기 전 확인사항

- [ ] `cargo build` 성공 (MSVC + RUSTFLAGS 설정된 shell에서)
- [ ] `cargo test` 17/17 통과
- [ ] `cargo run` 후 `/health`가 `ok` 반환
- [ ] Phase 2에서 추가할 것: `scripts/base` (기준 모듈), `scripts/extensions` (확장),
      Redis/PostgreSQL 연결, 모듈 로딩 파이프라인, CJ1101(구문오류) 감지

---

## 7. 참고 파일
- `docs/DEVELOPMENT_PLAN.md` — 전체 개발 계획 (Phase 0~5)
- `docs/STATUS.md` — 진행 상태
- `docs/rquickjs/API_CHANGES.md` — rquickjs 0.14 API 상세 (예제 코드 포함)
- `docs/setting/HANDOVER_phase_1_design.md` — Phase 1 설계 결정 (왜 이렇게 했는지)
- `docs/setting/HANDOVER_phase_1_env_setup.md` — 빌드 환경 상세 (cmake, MSYS2, junction)
- `docs/setting/handover_template.md` — 핸드오버 문서 템플릿
- `src/engine/runtime.rs` — Phase 1 핵심 코드 (Engine, SandboxedContext)
- `src/engine/throttle.rs` — Throttling 설정 + 에러 분류
