//! 인프라 에러 코드 체계.
//!
//! AGENTS.md의 6자리 에러 코드 명명 규칙을 따른다:
//! `[도메인 1자][R|J][종류 1자][개별번호 3자]`
//! - `C` = Core / Infrastructure 도메인
//! - `R` = Rust Core / 인프라 레이어
//! - `J` = JavaScript / 스크립트 및 샌드박스 레이어
//! - 세 번째 자리(종류): `0`=인프라, `1`=실행, `2`=소켓, `3`=데이터, `4`=보안

use serde::Serialize;
use std::fmt;

/// 인프라 에러 코드.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum InfraError {
    /// CR0001: 시스템 메모리 부족 또는 Tokio 스레드 풀 할당 실패 (C-인프라)
    Cr001,
    /// CR0002: Runtime / Context 생성 실패 (C-인프라)
    Cr002,
    /// CJ1102: 스크립트 실행 시간 초과 (CPU Timeout, 무한 루프 강제 종료) (C-실행)
    Cj102,
    /// CJ1103: 샌드박스 메모리 한도 초과 (Memory Limit Exceeded) (C-실행)
    Cj103,
    /// CJ1104: JS 스크립트 실행 중 일반 예외 (CPU/메모리 초과 아님) (C-실행)
    Cj104,
}

impl InfraError {
    /// 6자리 에러 코드 문자열.
    pub const fn code(self) -> &'static str {
        match self {
            InfraError::Cr001 => "CR0001",
            InfraError::Cr002 => "CR0002",
            InfraError::Cj102 => "CJ1102",
            InfraError::Cj103 => "CJ1103",
            InfraError::Cj104 => "CJ1104",
        }
    }

    /// 인간 가독 메시지.
    pub const fn message(self) -> &'static str {
        match self {
            InfraError::Cr001 => "시스템 메모리 부족 또는 스레드 할당 실패",
            InfraError::Cr002 => "rquickjs Runtime/Context 생성 실패",
            InfraError::Cj102 => "스크립트 실행 시간 초과 (CPU Timeout)",
            InfraError::Cj103 => "샌드박스 메모리 한도 초과",
            InfraError::Cj104 => "JS 스크립트 실행 예외 (일반 throw)",
        }
    }
}

impl fmt::Display for InfraError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code(), self.message())
    }
}

/// API 응답용으로 직렬화되는 에러 본문.
///
/// 스크립트 내부 예외와 동일한 `{ code, message }` 형식을 사용해
/// Rust/JS 레이어 간 에러 형식을 통일한다.
#[derive(Debug, Clone, Serialize)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
}

impl From<InfraError> for ErrorBody {
    fn from(e: InfraError) -> Self {
        Self {
            code: e.code().to_string(),
            message: e.message().to_string(),
        }
    }
}

impl std::error::Error for InfraError {}
