//! 샌드박스 리소스 제한 설정 및 에러 분류.
//!
//! CPU Timeout(CJ1102)과 Memory Limit(CJ1103)을 적용하고,
//! 발생된 rquickjs 에러를 인프라 에러 코드로 분류한다.

use crate::core::error::InfraError;

/// 샌드박스 리소스 제한 설정.
#[derive(Debug, Clone)]
pub struct ThrottleConfig {
    /// 메모리 제한 (바이트). 0 = 무제한.
    pub memory_limit: usize,
    /// 스택 제한 (바이트). 0 = 런타임 기본값(256KB).
    pub max_stack: usize,
    /// CPU 타임아웃 (밀리초). 0 = 제한 없음.
    pub cpu_timeout_ms: u64,
}

impl Default for ThrottleConfig {
    fn default() -> Self {
        Self {
            memory_limit: 8 * 1024 * 1024, // 8MB
            max_stack: 256 * 1024,         // 256KB (런타임 기본)
            cpu_timeout_ms: 1000,          // 1초
        }
    }
}

/// CPU 타임아웃 초과 여부 플래그.
///
/// `interrupt_handler` 클로저가 내부적으로 초과를 감지했는지 기록한다.
/// (rquickjs 0.14에서 OOM과 CPU Timeout이 모두 `Error::Exception`으로
/// 표출되므로, 분류를 위해 인위적 플래그가 필요하다.)
#[derive(Debug, Default)]
pub struct CpuExceeded(pub std::sync::atomic::AtomicBool);

impl CpuExceeded {
    pub fn mark(&self) {
        self.0.store(true, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn is_set(&self) -> bool {
        self.0.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn reset(&self) {
        self.0.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

/// rquickjs 에러를 인프라 에러 코드로 분류한다.
///
/// 분기 규칙 (Phase 1 design):
/// 1. `cpu_exceeded` 플래그 설정됨 -> CJ1102 (CPU Timeout)
/// 2. `memory_exceeded` (malloc_size >= malloc_limit) -> CJ1103 (메모리 초과)
/// 3. 그 외 `is_exception()` -> CJ1104 (일반 JS throw 예외)
/// 4. 비 예외 에러 -> CR0002 (인프라 레벨 생성/운영 오류)
pub fn classify_error(
    err: &rquickjs::Error,
    cpu_exceeded: &CpuExceeded,
    memory_exceeded: bool,
) -> InfraError {
    if cpu_exceeded.is_set() {
        return InfraError::Cj102;
    }
    if memory_exceeded {
        return InfraError::Cj103;
    }
    if err.is_exception() {
        // CPU/메모리 초과가 아닌 일반 JS 예외
        return InfraError::Cj104;
    }
    InfraError::Cr002
}
