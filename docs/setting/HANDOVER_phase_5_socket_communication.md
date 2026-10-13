# HANDOVER: Phase 5 — 소켓 통신 레이어

> 이 문서는 Phase 5를 완료하고 단절했을 때, 다음 개발자(또는 다음 세션의 AI 에이전트)가
> 이어서 작업을 진행할 수 있도록 작성된 핸드오버 문서입니다.

---

## 1. Phase 5 완료 내용

### 1.1 소켓 모듈 구조

```
src/socket/
├── mod.rs        # SocketConfig (SOCKET_MAX_SESSIONS 환경변수)
├── session.rs    # SessionPool + SocketSession (RwLock<HashMap>)
├── handler.rs    # SocketHandler — rquickjs 샌드박스에서 JS 훅 발동
└── ws.rs         # ws_handler — axum WebSocket upgrade + 연결 루프
```

### 1.2 WebSocket 플로우

```
클라이언트 (wscat/ws)
    │
    ▼
GET /ws (axum WebSocketUpgrade)
    │
    ├─ 1. 세션 풀 한도 확인 (CR2001)
    ├─ 2. 세션 생성 (socket_id = UUID)
    ├─ 3. WebSocket upgrade
    │
    ▼
handle_socket()
    │
    ├─ on_socket_connect 훅 발동 → welcome 메시지
    │
    └─ 루프:
        ├─ 인바운드: 클라이언트 메시지 → on_socket_message → reply
        └─ 아웃바운드: 서버 메시지 → 클라이언트로 송신
    │
    (연결 종료)
    ├─ on_socket_disconnect 훅 발동
    └─ 세션 풀에서 삭제
```

### 1.3 JS 훅 시그니처

```javascript
// scripts/socket_hooks.js

globalThis.on_socket_connect = function(payload) {
    // payload: { socket_id: "..." }
    return "welcome message";  // string 반환 (welcome 메시지)
};

globalThis.on_socket_message = function(payload) {
    // payload: { socket_id: "...", data: { ... } }
    return JSON.stringify({ type: "reply", data: ... });  // string 반환
    // return null;  // reply 없음
};

globalThis.on_socket_disconnect = function(payload) {
    // payload: { socket_id: "..." }
    return null;  // 반환값 무시
};
```

### 1.4 에러 매핑

| 코드 | 조건 | 반환 시점 |
|------|------|----------|
| `CR2001` | 세션 풀 제한 초과 | upgrade 전 |
| `CR2002` | WebSocket upgrade 실패 | upgrade 중 |
| `CJ2101` | JS 훅 throw 또는 시리얼라이즈 실패 | on_socket_message |
| `CJ1102` | JS 훅 CPU Timeout | on_socket_message |
| `CJ1103` | JS 훅 Memory Limit | on_socket_message |

### 1.5 환경변수

| 변수 | 기본값 | 설명 |
|------|--------|------|
| `SOCKET_MAX_SESSIONS` | `1000` | 동시 WebSocket 연결 최대 수 |

---

## 2. 테스트

```bash
cargo test --test phase5_socket
```

| 테스트 | 검증 |
|--------|------|
| `ws_connect_receives_welcome` | WebSocket upgrade + on_socket_connect → welcome |
| `ws_message_echo` | on_socket_message → reply |
| `ws_message_throw_returns_error_code` | on_socket_message throw → CJ2101 |

전체 테스트: **48개 통과** (Phase 1: 17, Phase 2: 10, Phase 3: 6, Phase 4: 11, Phase 5: 3, +1 lib test)

---

## 3. 주의사항

### 3.1 JS 훅 스크립트 — `var api` 금지

소켓 훅 스크립트는 Base/Extension 모듈이 아니므로, `var api = {}`를 사용할 수 없습니다.
`globalThis.on_socket_*` 함수를 직접 정의합니다.

### 3.2 `eval_value_as_string` vs `eval_void`

- `eval_void`: 훅 스크립트 바인딩용 (함수 선언, 반환값 무시)
- `eval_value_as_string`: 훅 발동용 (return 값 → `Option<String>`)

`eval_value_as_string`은 `Option<String>`으로 변환되므로, JS에서 `null`을 반환해야 `None`이 됩니다.
`undefined`를 반환하면 `None`이 됩니다.

### 3.3 `payload_json` 임베드 — 따옴표 이스케이프

`serde_json::to_string(payload)`의 결과를 JS에 임베드할 때, **`serde_json::to_string(&payload_json)`**으로
JSON 문자열 리터럴로 변환합니다. 따옴표가 포함되므로, 직접 임베드 시 JS 구문이 깨집니다.

```rust
// ❌ 잘못됨: payload_json에 따옴표가 포함되면 JS 구문 오류
let script = format!("var payload = JSON.parse({payload_json});");

// ✅ 올바름: JSON 문자열 리터럴로 안전하게 임베드
let payload_js = serde_json::to_string(&payload_json).unwrap_or_default();
let script = format!("var payload = JSON.parse({payload_js});");
```

### 3.4 `SocketHandler` — 매 훅 호출마다 새 샌드박스

`SocketHandler`는 **매 훅 호출마다** `create_sandbox()`를 호출합니다.
이는 **격리**를 보장하지만, **성능상 비효율**입니다.
향후 `SandboxedContext` 풀링을 고려할 수 있습니다.

### 3.5 `SessionPool` — `RwLock<HashMap>`

`SessionPool`은 `RwLock<HashMap<String, SocketSession>>`을 사용합니다.
**동시성**: 읽기(개수 확인)는 `read()`, 쓰기(추가/삭제)는 `write()`.
**한계**: `RwLock`은 고스압 상황에서 성능 저하가 있을 수 있습니다.
대규모 환경에서 `DashMap` 또는 `tokio::sync::RwLock`을 고려할 수 있습니다.

---

## 4. 다음 단계 (Phase 6)

1. **Phase 6.1: 다층 캐시 완전 연동**
   - L1 ↔ L2 (Redis) ↔ L3 (PostgreSQL) 완전 연동
   - Redis Pub/Sub으로 L1 무효화 → L2 조회 → L3 조회

2. **Phase 6.2: 성능 벤치마크**
   - 동시 WebSocket 100개 연결 테스트
   - 훅 발동 성능 (ms/회)
   - 세션 풀 한계 테스트

3. **Phase 6.3: 최종 통합 테스트**
   - 전체 Phase 1~6 통합 시나리오
   - PostgreSQL + Redis 인스턴스 실연동

4. **Phase 6.4: 문서화**
   - API 문서 (WebSocket protocol)
   - 운영 가이드 (SOCKET_MAX_SESSIONS 조정)

---

## 5. 빌드 및 테스트

```bash
source /c/temp/load_msenv.sh
export RUSTFLAGS='-C link-arg=/LIBPATH:C:\wsdk10\10.0.26100.0\um\x64 -C link-arg=/LIBPATH:C:\wsdk10\10.0.26100.0\ucrt\x64'
cargo test
```

전체 48개 테스트 통과 확인:
```
test result: ok. 48 passed; 0 failed
```
