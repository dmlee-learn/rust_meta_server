//! Phase 5: 소켓 통신 통합 테스트.
//!
//! axum WebSocket + JS 훅 발동 검증.
//!
//! 테스트 케이스:
//! 1. WebSocket upgrade 성공 → welcome 메시지
//! 2. on_socket_message 훅 발동 → reply
//! 3. on_socket_message throw → CJ2101

use futures::{SinkExt, StreamExt};
use meta_server::engine::{Engine, ThrottleConfig};
use meta_server::http::build_router;
use meta_server::socket::SocketConfig;
use tokio::net::TcpListener;

/// 테스트용 훅 스크립트.
const HOOK_SCRIPT: &str = r#"
globalThis.on_socket_connect = function(payload) {
    return "welcome_to_socket";
};

globalThis.on_socket_message = function(payload) {
    if (payload.data && payload.data.action === "echo") {
        return JSON.stringify({
            type: "reply",
            socket_id: payload.socket_id,
            echo: payload.data.value
        });
    }
    if (payload.data && payload.data.action === "throw") {
        throw new Error(JSON.stringify({ code: "SJ1201", message: "socket 훅 throw" }));
    }
    return null;
};

globalThis.on_socket_disconnect = function(payload) {
    return null;
};
"#;

/// 테스트 서버 기동 → (url, server_handle)
async fn start_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    let engine = Engine::new(ThrottleConfig {
        memory_limit: 16 * 1024 * 1024,
        max_stack: 0,
        cpu_timeout_ms: 1500,
    })
    .unwrap();

    let socket_config = SocketConfig {
        max_sessions: 100,
    };

    let router = build_router(
        engine,
        None,
        vec![],
        socket_config,
        HOOK_SCRIPT.to_string(),
        "memory".into(),
        vec![],
        false,
        "test-secret".into(),
        true,
        1000,
    );

    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });

    format!("ws://127.0.0.1:{port}/ws")
}

/// WebSocket 클라이언트 기동.
async fn connect_ws(
    url: &str,
) -> tokio_tungstenite::WebSocketStream<
    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
> {
    let (ws, _response) = tokio_tungstenite::connect_async(url)
        .await
        .unwrap();
    ws
}

/// welcome 메시지 수신.
async fn receive_welcome(
    ws: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> String {
    loop {
        match ws.next().await {
            Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text))) => {
                let v: serde_json::Value = serde_json::from_str(&text).unwrap();
                if v["type"] == "welcome" {
                    return text;
                }
            }
            Some(Ok(_)) => continue,
            Some(Err(e)) => panic!("WS 수신 오류: {e}"),
            None => panic!("WS 연결 종료"),
        }
    }
}

/// 메시지 전송 + reply 수신.
async fn send_and_receive(
    ws: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    payload: &serde_json::Value,
) -> serde_json::Value {
    ws.send(tokio_tungstenite::tungstenite::Message::Text(
        payload.to_string(),
    ))
    .await
    .unwrap();

    loop {
        match ws.next().await {
            Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text))) => {
                return serde_json::from_str(&text).unwrap();
            }
            Some(Ok(_)) => continue,
            Some(Err(e)) => panic!("WS 수신 오류: {e}"),
            None => panic!("WS 연결 종료"),
        }
    }
}

#[tokio::test]
async fn ws_connect_receives_welcome() {
    let url = start_server().await;
    let mut ws = connect_ws(&url).await;
    let welcome = receive_welcome(&mut ws).await;
    let v: serde_json::Value = serde_json::from_str(&welcome).unwrap();
    assert_eq!(v["type"], "welcome");
    assert!(v["data"].to_string().contains("welcome_to_socket"));
}

#[tokio::test]
async fn ws_message_echo() {
    let url = start_server().await;
    let mut ws = connect_ws(&url).await;
    let _ = receive_welcome(&mut ws).await;

    let payload = serde_json::json!({
        "action": "echo",
        "value": "hello"
    });
    let reply = send_and_receive(&mut ws, &payload).await;
    assert_eq!(reply["type"], "reply");
    assert_eq!(reply["echo"], "hello");
}

#[tokio::test]
async fn ws_message_throw_returns_error_code() {
    let url = start_server().await;
    let mut ws = connect_ws(&url).await;
    let _ = receive_welcome(&mut ws).await;

    let payload = serde_json::json!({
        "action": "throw"
    });
    let reply = send_and_receive(&mut ws, &payload).await;
    assert_eq!(reply["code"], "CJ2101");
}
