/**
 * 소켓 훅 샘플 스크립트 (Phase 5).
 *
 * 서버 기동 시 이 스크립트가 rquickjs에 바인딩되고,
 * WebSocket 이벤트 발생 시 각 훅이 발동한다.
 *
 * 훅 시그니처:
 * - on_socket_connect(payload) → string | undefined
 * - on_socket_message(payload) → string | undefined
 * - on_socket_disconnect(payload) → undefined
 */

/**
 * WebSocket 연결 시 발동.
 *
 * @param {object} payload - { socket_id: string }
 * @returns {string|undefined} welcome message 또는 undefined
 */
function on_socket_connect(payload) {
    return "연결 확인: " + payload.socket_id;
}

/**
 * WebSocket 메시지 수신 시 발동.
 *
 * @param {object} payload - { socket_id: string, data: object }
 * @returns {string|undefined} reply message 또는 undefined
 */
function on_socket_message(payload) {
    // data.type 이 "ping" 이면 "pong" 응답
    if (payload.data && payload.data.type === "ping") {
        return "pong";
    }

    // data.type 이 "echo" 이면 원문 반환
    if (payload.data && payload.data.type === "echo") {
        return JSON.stringify({ type: "echo_reply", data: payload.data.payload });
    }

    // 그 외: 무시
    return undefined;
}

/**
 * WebSocket 단절 시 발동.
 *
 * @param {object} payload - { socket_id: string }
 */
function on_socket_disconnect(payload) {
    // console.log 불가 (rquickjs) — 무시
}
