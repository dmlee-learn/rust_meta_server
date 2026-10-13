//! 소켓 통신 레이어 (Phase 5).
//!
//! AGENTS.md 철학: 인프라 제어권은 Rust가 가진다.
//! WebSocket 연결 관리 + JS 훅 발동(on_socket_connect/message/disconnect).
//!
//! 에러 코드:
//! - CR2001: 세션 풀 제한 초과
//! - CR2002: WebSocket 핸드셰이크 실패
//! - CJ2101: 소켓 훅 시리얼라이즈/패킷 손실

pub mod handler;
pub mod session;
pub mod ws;

/// 소켓 레이어 설정.
#[derive(Debug, Clone)]
pub struct SocketConfig {
    /// 최대 동시 세션 수. 기본값 1000.
    /// 환경변수 `SOCKET_MAX_SESSIONS`에서 읽음.
    pub max_sessions: usize,
}

impl Default for SocketConfig {
    fn default() -> Self {
        Self {
            max_sessions: 1000,
        }
    }
}

impl SocketConfig {
    /// 환경변수에서 설정 생성.
    pub fn from_env() -> Self {
        let max_sessions = std::env::var("SOCKET_MAX_SESSIONS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1000);
        Self { max_sessions }
    }
}
