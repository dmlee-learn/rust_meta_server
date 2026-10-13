//! rquickjs 런타임 초기화 및 샌드박스 리소스 제한 (Throttling) 관리.
//!
//! AGENTS.md 철학: 인프라 제어권은 Rust가 가진다.
//! 모든 JS 컨텍스트는 CPU Timeout과 Memory Limit 이중 제한 아래 생성된다.

pub mod loader;
pub mod runtime;
pub mod throttle;

pub use loader::{HOOK_NAMES, ModuleLoader};
pub use runtime::{Engine, SandboxedContext};
pub use throttle::ThrottleConfig;

#[allow(unused_imports)]
pub use throttle::classify_error;
