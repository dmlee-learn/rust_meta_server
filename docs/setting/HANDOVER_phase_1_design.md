# Phase 1 이어서 개발하기 (Infrastructure Skeleton Handover)

> **대상:** Phase 1 (인프라 뼈대)를 **중단했다가 이어서 진행하는** 누구든 (AI 에이전트 또는 개발자).
> 이 문서를 읽으면 Phase 1의 전체 목표, 아키텍처, 그리고 "지금부터 어디부터"를 정확히 파악할 수 있습니다.

---

## 1. Phase 1 목표 (Goal)

Axum 서버가 기동하면 rquickjs 런타임이 즉시 초기화되고, 모든 JS 컨텍스트가 **CPU Timeout**과 **Memory Limit**이라는 이중 제한(Throttling) 아래 생성되도록 기초 코드를 완성하는 것이다.

**핵심 원칙 (AGENTS.md 준수):**
- 인프라 제어권은 Rust가 가진다. (`src/engine`는 순수 Rust만)
- `unwrap()`, `panic!` 절대 금지. 반드시 `Result<T, rquickjs::Error>` 반환.
- 에러 코드 체계: `CR0001` (메모리/스레드 부족), `CJ1102` (JS 실행 시간 초과), `CJ1103` (샌드박스 메모리 초과)

## 2. 기대 폴더 구조 (Expected Layout)

```
src/
├── main.rs          # Tokio 런타임 기동 + Axum 서버 바인딩
├── core/
│   └── mod.rs       # Axum 인프라, 요청 라우팅骨架
└── engine/
    ├── mod.rs       # 런타임 초기화 진입점
    ├── runtime.rs   # rquickjs Runtime 생성 및 리소스 제한 강제
    └── error.rs     # CR0001/CJ1102/CJ1103 에러 코드 매핑
```

## 3. 핵심 구현 스펙 (Implementation Spec)

### 3.1 의존성 (Cargo.toml)
```toml
[dependencies]
axum = "0.7"
tokio = { version = "1", features = ["full"] }
rquickjs = { version = "0.14", features = ["bindgen", "classes"] }
serde = { version = "1", features = ["derive"] }
```
> **주의:** rquickjs API는 버전별로 breaking change가 큼. 세션 시작 시 `cargo tree | grep rquickjs`로 버전 확인 필수.

### 3.2 `src/engine/runtime.rs` - 런타임 생성
```rust
use rquickjs::Runtime;
use std::result::Result;

pub struct SandboxLimits {
    pub cpu_timeout_ms: u64,   // 예: 1000ms
    pub memory_limit_mb: u64,  // 예: 8MB
}

impl SandboxLimits {
    pub fn default() -> Self {
        Self { cpu_timeout_ms: 1000, memory_limit_mb: 8 }
    }
}

pub fn create_sandboxed_runtime(limits: &SandboxLimits) -> Result<Runtime, rquickjs::Error> {
    let mut rt = Runtime::new().map_err(|e| rquickjs::Error::from_value(rquickjs::Exception::from(rquickjs::Value::from(e.to_string()))))?;
    
    // CPU Timeout 강제: 무한 루프(CJ1102) 방어
    rt.set_cpu_timeout(std::time::Duration::from_millis(limits.cpu_timeout_ms));
    
    // Memory Limit 강제: 샌드박스 제한(CJ1103) 방어
    // (rquickjs 버전 API에 따라 다를 수 있음 - 확인 필수)
    
    Ok(rt)
}
```
> **체크포인트:** `set_cpu_timeout`은 무한 루프가 발생한 컨텍스트를 강제 종료(CJ1102)하도록 하는 핵심 API다. 이 값이 없으면 악성 스크립트가 전체 Tokio 스레드 풀을 소진하게 된다.

### 3.3 `src/main.rs` - 서버 기동 시 런타임 초기화
```rust
use tokio::net::TcpListener;
use axum::Router;
use crate::engine::runtime::{create_sandboxed_runtime, SandboxLimits};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Phase 1: 서버 기동 시 rquickjs 런타임 초기화
    let limits = SandboxLimits::default();
    let _runtime = create_sandboxed_runtime(&limits)
        .map_err(|e| format!("CR0001: Runtime init failed: {e}"))?;
    
    // Axum 서버 바인딩
    let app = Router::new();
    let listener = TcpListener::bind("0.0.0.0:3000").await?;
    println!("[MetaServer] rquickjs engine initialized, serving on port 3000");
    axum::serve(listener, app).await?;
    
    Ok(())
}
```

## 4. 검증 테스트 (Verification)

Phase 1 완료 후 아래 테스트가 통과해야 한다:

| 테스트 | 방법 | 기대 결과 |
|--------|------|-----------|
| 서버 기동 | `cargo run` | `[MetaServer] rquickjs engine initialized` 로그 출력 |
| CPU Timeout | `while(true){}` 스크립트 실행 | ~1초 후 강제 종료, CJ1102 에러 |
| Memory Limit | 무한 배열 할당 스크립트 실행 | 8MB 초과 시 강제 종료, CJ1103 에러 |
| 정상 실행 | `1+1` 스크립트 실행 | `2` 반환, 에러 없음 |

## 5. 다음 단계 (Handover to Phase 2)

Phase 1이 완료되면 `docs/STATUS.md`의 `WIP 체크포인트`를 아래와 같이 갱신하고 Phase 2로 이동한다:

```
### 현재 상태
- 진행 중 Task: P1 완료 - Phase 2.1 (SDK 함수) 착수 대기
- 다음 액션:
  1. src/sdk/ 폴더 생성
  2. rust_calculate_base_tax() 첫 SDK 함수 구현
  3. rquickjs bindgen으로 JS 컨텍스트 주입 구조 설계
```
