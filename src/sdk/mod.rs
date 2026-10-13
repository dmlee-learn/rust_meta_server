//! SDK: 스크립트 내부에서 호출할 수 있는 Rust 고성능 함수 자산.
//!
//! AGENTS.md: `src/sdk` — rquickjs에 바인딩하여 JS 레이어로 노출하는 Rust 함수.
//! - `rust_` 접두사 필수
//! - panic 금지, 반드시 `Result<T, rquickjs::Error>` 반환
//! - 음수/무효 입력 시 에러 코드 JSON 반환

pub mod math;
pub mod utils;
