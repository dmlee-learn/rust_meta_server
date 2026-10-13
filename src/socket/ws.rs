//! WebSocket 연결 관리 + JS 훅 발동.
//!
//! axum WebSocketUpgrade로 upgrade, incoming/outgoing 메시지 루프.
//!
//! - incoming: 클라이언트 → 서버 (JSON 파싱 → JS 훅 발동)
//! - outgoing: 서버 → 클라이언트 (JS 훅 반환값, welcome message)
//!
//! 에러 코드:
//! - CR2001: 세션 풀 제한 초과
//! - CR2002: WebSocket upgrade 실패
//! - CJ2101: JS 훅 시리얼라이즈/패킷 손실

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::Response;
use futures::{SinkExt, StreamExt};
use serde_json::Value;
use std::sync::Arc;

use crate::socket::handler::SocketHandler;
use crate::socket::session::{SessionPool, SocketSession};

/// WebSocket upgrade 핸들러 (`GET /ws`).
///
/// - 세션 풀 한도 확인 (CR2001)
/// - WebSocket upgrade
/// - incoming/outgoing 루프 기동
/// Phase 21: domain query parameter 지원.
pub async fn ws_handler(
    State(state): State<crate::http::AppState>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
    ws: WebSocketUpgrade,
) -> Response {
    let pool = state.socket_pool.clone();
    let handler = state.socket_handler.clone();

    // Phase 21: domain 추출
    let domain = params.get("domain").cloned().unwrap_or_default();

    // 1. 세션 풀 한도 확인
    if let Err(e) = pool.check_capacity().await {
        let body = serde_json::json!({
            "code": "CR2001",
            "message": e,
        });
        return axum::http::Response::builder()
            .status(axum::http::StatusCode::TOO_MANY_REQUESTS)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(axum::body::Body::from(body.to_string()))
            .expect("응답 생성");
    }

    // 2. 세션 생성
    let (session, mut rx) = match pool.add_session().await {
        Ok(v) => v,
        Err(e) => {
            let body = serde_json::json!({
                "code": "CR2001",
                "message": e,
            });
            return axum::http::Response::builder()
                .status(axum::http::StatusCode::TOO_MANY_REQUESTS)
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(body.to_string()))
                .expect("응답 생성");
        }
    };

    let socket_id = session.socket_id.clone();

    // 3. WebSocket upgrade
    ws.on_upgrade(move |socket| handle_socket(socket, session, rx, handler, pool, socket_id, domain))
}

/// WebSocket 연결 처리 루프.
async fn handle_socket(
    socket: WebSocket,
    session: SocketSession,
    mut rx: tokio::sync::mpsc::UnboundedReceiver<String>,
    handler: SocketHandler,
    pool: Arc<SessionPool>,
    socket_id: String,
    domain: String,
) {
    let (mut ws_tx, mut ws_rx) = socket.split();

    // on_socket_connect 훅 발동 (동기)
    let connect_payload = serde_json::json!({
        "socket_id": socket_id,
        "domain": domain,
    });
    if let Some(welcome) = handler.on_socket_connect(&connect_payload) {
        let msg = serde_json::json!({
            "type": "welcome",
            "domain": domain,
            "data": welcome,
        });
        let _ = ws_tx.send(Message::Text(msg.to_string())).await;
    }

    // 인바운드/아웃바운드 통합 루프
    loop {
        tokio::select! {
            // 인바운드: 클라이언트 메시지 수신
            msg = ws_rx.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        // JSON 파싱
                        let payload: Value = match serde_json::from_str(&text) {
                            Ok(v) => v,
                            Err(e) => {
                                let err = serde_json::json!({
                                    "code": "CJ2101",
                                    "message": format!("메시지 파싱 실패: {e}"),
                                });
                                let _ = ws_tx.send(Message::Text(err.to_string())).await;
                                continue;
                            }
                        };

                        // on_socket_message 훅 발동 (동기)
                        match handler.on_socket_message(&socket_id, &payload) {
                            Ok(Some(reply)) => {
                                let _ = ws_tx.send(Message::Text(reply)).await;
                            }
                            Ok(None) => {
                                // reply 없음: 무시
                            }
                            Err(code) => {
                                let err = serde_json::json!({
                                    "code": code,
                                    "message": "소켓 훅 실행 오류",
                                });
                                let _ = ws_tx.send(Message::Text(err.to_string())).await;
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) => {
                        break;
                    }
                    Some(Ok(_)) => {
                        // Binary, Ping, Pong 무시
                    }
                    Some(Err(_)) | None => {
                        break;
                    }
                }
            }
            // 아웃바운드: 서버 메시지 송신
            out_msg = rx.recv() => {
                match out_msg {
                    Some(msg) => {
                        if let Err(e) = ws_tx.send(Message::Text(msg)).await {
                            eprintln!("[Socket]出站 송신 실패 ({socket_id}): {e}");
                            break;
                        }
                    }
                    None => {
                        // 채널 종료
                        break;
                    }
                }
            }
        }
    }

    // on_socket_disconnect 훅 발동 (동기)
    let disconnect_payload = serde_json::json!({
        "socket_id": socket_id,
    });
    handler.on_socket_disconnect(&disconnect_payload);

    // 세션 풀에서 삭제
    pool.remove_session(&socket_id).await;

    println!("[Socket] 세션 종료: {socket_id}");
}

/// Phase 23: Admin 대시보드 실시간 WebSocket.
///
/// `GET /ws/admin` — 5초 간격으로 metrics JSON push.
/// JWT/Rate Limit 미들웨어 제외.
pub async fn ws_admin_handler(
    State(state): State<crate::http::AppState>,
    ws: WebSocketUpgrade,
) -> Response {
    let state_clone = state.clone();
    ws.on_upgrade(move |socket| handle_admin_ws(socket, state_clone))
}

/// Admin WebSocket 연결 처리: 5초 간격 metrics push.
async fn handle_admin_ws(
    socket: WebSocket,
    state: crate::http::AppState,
) {
    let (mut ws_tx, mut ws_rx) = socket.split();

    // welcome message
    let welcome = serde_json::json!({
        "type": "welcome",
        "message": "Admin WebSocket connected. Metrics will be pushed every 5s."
    });
    let _ = ws_tx.send(Message::Text(welcome.to_string())).await;

    // 5초 간격 metrics push 루프
    loop {
        tokio::select! {
            // 5초 타이머
            _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {
                let usage = state.engine.memory_usage();
                let m = &state.metrics;
                use std::sync::atomic::Ordering;

                let metrics_json = serde_json::json!({
                    "type": "metrics",
                    "data": {
                        "http_requests": {
                            "board": m.http_requests.board.load(Ordering::SeqCst),
                            "erp": m.http_requests.erp.load(Ordering::SeqCst),
                            "blog": m.http_requests.blog.load(Ordering::SeqCst),
                            "shop": m.http_requests.shop.load(Ordering::SeqCst)
                        },
                        "cache": {
                            "hits": m.cache_hits.l1.load(Ordering::SeqCst)
                                + m.cache_hits.l2.load(Ordering::SeqCst)
                                + m.cache_hits.l3.load(Ordering::SeqCst),
                            "misses": m.cache_misses.l1.load(Ordering::SeqCst)
                                + m.cache_misses.l2.load(Ordering::SeqCst)
                                + m.cache_misses.l3.load(Ordering::SeqCst)
                        },
                        "hooks": {
                            "executions": m.hook_executions.load(Ordering::SeqCst),
                            "errors": m.hook_errors.load(Ordering::SeqCst)
                        },
                        "engine_memory_mb": usage.malloc_size as f64 / 1024.0 / 1024.0,
                        "active_sessions": state.socket_pool.len()
                    }
                });
                if let Err(_) = ws_tx.send(Message::Text(metrics_json.to_string())).await {
                    break;
                }
            }
            // 클라이언트 메시지 수신 (ping/pong 또는 close)
            msg = ws_rx.next() => {
                match msg {
                    Some(Ok(Message::Close(_))) => break,
                    Some(Ok(Message::Text(_))) => {
                        // 클라이언트에서 보내는 메시지는 무시 (admin WS는 read-only)
                    }
                    Some(Ok(_)) => {}
                    Some(Err(_)) | None => break,
                }
            }
        }
    }

    println!("[Socket] Admin WebSocket 세션 종료");
}
