//! 소켓 훅 발동 로직: JS `on_socket_connect/message/disconnect` 호출.
//!
//! rquickjs 샌드박스에서 JS 훅을 실행하고, 반환값을出站 메시지로 전달.
//!
//! 훅 시그니처:
//! - `on_socket_connect(payload) → string | undefined`
//! - `on_socket_message(payload) → string | undefined`
//! - `on_socket_disconnect(payload) → undefined`
//!
//! 에러 코드:
//! - CJ2101: JS 훅 throw 또는 시리얼라이즈 실패

use serde_json::Value as JsonValue;

use crate::engine::Engine;

/// 소켓 훅 핸들러: 엔진 + 훅 스크립트 소스.
#[derive(Clone)]
pub struct SocketHandler {
    engine: Engine,
    /// `on_socket_*` 훅을 정의하는 JS 스크립트 소스.
    /// `scripts/socket_hooks.js` 또는 내장 기본값.
    script_source: String,
}

impl SocketHandler {
    pub fn new(engine: Engine, script_source: String) -> Self {
        Self {
            engine,
            script_source,
        }
    }

    /// 샌드박스 생성 + 훅 스크립트 바인딩.
    fn create_sandbox(&self) -> Result<crate::engine::SandboxedContext, String> {
        let sb = self
            .engine
            .create_sandbox()
            .map_err(|e| format!("CR0002: {e}"))?;
        // 훅 스크립트 실행: on_socket_* 함수를 globalThis에 바인딩
        // 반환값 무시 (함수 선언 스크립트)
        if let Err(e) = sb.eval_void(&self.script_source) {
            return Err(format!("CJ1101: {e}"));
        }
        Ok(sb)
    }

    /// `on_socket_connect` 훅 발동.
    ///
    /// 반환: `Some(reply_string)` 또는 `None`.
    pub fn on_socket_connect(&self, payload: &JsonValue) -> Option<String> {
        let sb = match self.create_sandbox() {
            Ok(sb) => sb,
            Err(e) => {
                eprintln!("[Socket] 샌드박스 생성 실패: {e}");
                return None;
            }
        };

        let payload_json = serde_json::to_string(payload).unwrap_or_default();
        // JS 문자열 리터럴로 안전하게 임베드 (따옴표 이스케이프)
        let payload_js = serde_json::to_string(&payload_json).unwrap_or_default();
        let script = format!(
            "(function() {{\n\
             var payload = JSON.parse({payload_js});\n\
             if (typeof globalThis.on_socket_connect === 'function') {{\n\
             var result = globalThis.on_socket_connect(payload);\n\
             if (result === undefined || result === null) return null;\n\
             if (typeof result === 'object') return JSON.stringify(result);\n\
             return String(result);\n\
             }}\n\
             return null;\n\
             }})();"
        );

        match sb.eval_value_as_string(&script) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[Socket] on_socket_connect 실행 실패: {e}");
                None
            }
        }
    }

    /// `on_socket_message` 훅 발동.
    ///
    /// 반환: `Ok(Some(reply_string))`, `Ok(None)`, 또는 `Err(error_code)`.
    pub fn on_socket_message(
        &self,
        socket_id: &str,
        payload: &JsonValue,
    ) -> Result<Option<String>, String> {
        let sb = match self.create_sandbox() {
            Ok(sb) => sb,
            Err(e) => return Err(e),
        };

        let enriched = serde_json::json!({
            "socket_id": socket_id,
            "data": payload,
        });
        let payload_json = serde_json::to_string(&enriched).unwrap_or_default();
        // JS 문자열 리터럴로 안전하게 임베드 (따옴표 이스케이프)
        let payload_js = serde_json::to_string(&payload_json).unwrap_or_default();
        let script = format!(
            "(function() {{\n\
             var payload = JSON.parse({payload_js});\n\
             if (typeof globalThis.on_socket_message === 'function') {{\n\
             var result = globalThis.on_socket_message(payload);\n\
             if (result === undefined || result === null) return null;\n\
             if (typeof result === 'object') return JSON.stringify(result);\n\
             return String(result);\n\
             }}\n\
             return null;\n\
             }})();"
        );

        match sb.eval_value_as_string(&script) {
            Ok(v) => Ok(v),
            Err(e) => {
                let msg = e.to_string();
                let code = if msg.contains("CJ1102") {
                    "CJ1102"
                } else if msg.contains("CJ1103") {
                    "CJ1103"
                } else {
                    "CJ2101"
                };
                Err(code.to_string())
            }
        }
    }

    /// `on_socket_disconnect` 훅 발동.
    ///
    /// 반환값 무시 (단절 이벤트).
    pub fn on_socket_disconnect(&self, payload: &JsonValue) {
        let sb = match self.create_sandbox() {
            Ok(sb) => sb,
            Err(e) => {
                eprintln!("[Socket] 샌드박스 생성 실패 (disconnect): {e}");
                return;
            }
        };

        let payload_json = serde_json::to_string(payload).unwrap_or_default();
        // JS 문자열 리터럴로 안전하게 임베드 (따옴표 이스케이프)
        let payload_js = serde_json::to_string(&payload_json).unwrap_or_default();
        let script = format!(
            "(function() {{\n\
             var payload = JSON.parse({payload_js});\n\
             if (typeof globalThis.on_socket_disconnect === 'function') {{\n\
             return globalThis.on_socket_disconnect(payload);\n\
             }}\n\
             return undefined;\n\
             }})();"
        );

        if let Err(e) = sb.eval_value_as_string(&script) {
            eprintln!("[Socket] on_socket_disconnect 실행 실패: {e}");
        }
    }
}
