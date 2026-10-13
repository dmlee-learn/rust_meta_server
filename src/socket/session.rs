//! 소켓 세션 관리: SessionPool + SocketSession.
//!
//! `RwLock<HashMap>`으로 세션 관리:
//! - 읽기(메시지 라우팅): `read()`
//! - 쓰기(연결/단절): `write()`
//!
//! `socket_id`는 외부 공개용 UUID. 내부 `mpsc::Sender`로出站 메시지 전달.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{mpsc, RwLock};

use crate::socket::SocketConfig;

/// 소켓 세션: WebSocket 연결의 상태와出站 메시지 채널.
#[derive(Clone)]
pub struct SocketSession {
    /// 외부 공개용 세션 ID (UUID).
    pub socket_id: String,
    ///出站 메시지 송신 채널 (JSON 문자열).
    sender: mpsc::UnboundedSender<String>,
}

impl SocketSession {
    /// 세션에서出站 메시지를 비동기로 송신.
    /// 채널이 닫혔으면 `Err` 반환 (클라이언트 단절).
    pub fn send(&self, message: &str) -> Result<(), mpsc::error::SendError<String>> {
        self.sender.send(message.to_string())
    }
}

/// 세션 풀: `socket_id → SocketSession` 매핑 + 세션 수 제한.
pub struct SessionPool {
    sessions: RwLock<HashMap<String, SocketSession>>,
    /// 최대 세션 수.
    max_sessions: usize,
    /// 활성 세션 수 카운터 (sync 접근용).
    active_count: std::sync::atomic::AtomicUsize,
}

impl SessionPool {
    pub fn new(config: &SocketConfig) -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
            max_sessions: config.max_sessions,
            active_count: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// 세션 수 제한을 확인하고, 한도가 아니면 `Err(CR2001)` 반환.
    pub async fn check_capacity(&self) -> Result<(), String> {
        let sessions = self.sessions.read().await;
        if sessions.len() >= self.max_sessions {
            return Err(format!(
                "CR2001: 세션 풀 제한 초과 ({}/{})",
                sessions.len(),
                self.max_sessions
            ));
        }
        Ok(())
    }

    /// 새 세션 등록. `socket_id` 생성 + `Sender`/`Receiver` 쌍 생성.
    ///
    /// 반환: `(SocketSession, mpsc::UnboundedReceiver<String>)`
    /// - `SocketSession`:出站 메시지를 보낼 때 사용
    /// - `Receiver`:出站 메시지 루프에서 소비
    pub async fn add_session(&self) -> Result<(SocketSession, mpsc::UnboundedReceiver<String>), String> {
        // 세션 수 제한 확인
        self.check_capacity().await?;

        let socket_id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = mpsc::unbounded_channel::<String>();

        let session = SocketSession {
            socket_id: socket_id.clone(),
            sender: tx,
        };

        let mut sessions = self.sessions.write().await;
        sessions.insert(socket_id, session.clone());
        self.active_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        Ok((session, rx))
    }

    /// 세션 삭제 (단절 시).
    pub async fn remove_session(&self, socket_id: &str) {
        let mut sessions = self.sessions.write().await;
        sessions.remove(socket_id);
        self.active_count.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }

    /// 특정 세션 조회 (메시지 라우팅 시).
    pub async fn get_session(&self, socket_id: &str) -> Option<SocketSession> {
        let sessions = self.sessions.read().await;
        sessions.get(socket_id).cloned()
    }

    /// 현재 활성 세션 수.
    pub async fn session_count(&self) -> usize {
        let sessions = self.sessions.read().await;
        sessions.len()
    }

    /// 현재 활성 세션 수 (sync). health 엔드포인트용.
    pub fn len(&self) -> usize {
        self.active_count.load(std::sync::atomic::Ordering::SeqCst)
    }
}

/// `SessionPool`를 `Arc`로 감싸 공유.
pub type SessionPoolRef = Arc<SessionPool>;
