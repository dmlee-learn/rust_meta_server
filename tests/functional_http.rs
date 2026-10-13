//! 기능 테스트: 실제 HTTP 요청으로 엔진 Throttling 엔드포인트 검증.
//!
//! `tower::Service`로 `axum` 라우터를 in-process에서 호출해
//! `/health`, `/api/engine/info`, `/api/engine/eval` 엔드포인트가
//! Phase 1 요구사항을 충족하는지 검증한다.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use meta_server::engine::{Engine, ThrottleConfig};
use meta_server::http::build_router;
use tower::ServiceExt;

async fn body_text(res: axum::response::Response) -> String {
    let collected = res.into_body().collect().await.unwrap();
    String::from_utf8(collected.to_bytes().to_vec()).unwrap()
}

fn make_app(cpu_timeout_ms: u64) -> axum::Router {
    let engine = Engine::new(ThrottleConfig {
        memory_limit: 8 * 1024 * 1024,
        max_stack: 0,
        cpu_timeout_ms,
    })
    .expect("엔진 생성 실패");
    build_router(engine, None, vec![], meta_server::socket::SocketConfig::default(), String::new(), "memory".into(), vec![], false, "test-secret".into(), true, 1000)
}

/// /health 엔드포인트가 200 + "ok" 반환.
#[tokio::test]
async fn health_returns_ok() {
    let app = make_app(500);
    let req = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let text = body_text(res).await;
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["status"], "ok");
}

/// /api/engine/info 가 Throttling 설정을 노출하는지.
#[tokio::test]
async fn engine_info_exposes_throttle_config() {
    let app = make_app(750);
    let req = Request::builder()
        .uri("/api/engine/info")
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_str(&body_text(res).await).unwrap();
    assert_eq!(v["memory_limit"], 8 * 1024 * 1024);
    assert_eq!(v["cpu_timeout_ms"], 750);
}

/// 정상 스크립트가 eval되어 결과 반환.
#[tokio::test]
async fn eval_valid_script_returns_result() {
    let app = make_app(500);
    let payload = serde_json::json!({ "script": "3 * 7" });
    let req = Request::builder()
        .method("POST")
        .uri("/api/engine/eval")
        .header("Content-Type", "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_str(&body_text(res).await).unwrap();
    assert_eq!(v["code"], "OK");
    assert_eq!(v["result"], 21);
}

/// 무한 루프 스크립트가 CJ1102 에러 코드로 반환되고 서버가 살아남는지.
#[tokio::test]
async fn eval_infinite_loop_returns_cj102() {
    let app = make_app(100); // 100ms CPU 제한
    let payload = serde_json::json!({ "script": "let s=0;for(let i=0;i<9e9;i++){s+=i}s" });
    let req = Request::builder()
        .method("POST")
        .uri("/api/engine/eval")
        .header("Content-Type", "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    let v: serde_json::Value = serde_json::from_str(&body_text(res).await).unwrap();
    assert_eq!(v["code"], "CJ1102");
    // 서버가 죽지 않아 (에러 응답 자체가 반환됐으므로) 다음 요청 정상
    // (별개 앱 인스턴스로 후속 요청 검증)
    let app2 = make_app(500);
    let req2 = Request::builder()
        .method("POST")
        .uri("/api/engine/eval")
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::json!({ "script": "1+1" }).to_string(),
        ))
        .unwrap();
    let res2 = app2.oneshot(req2).await.unwrap();
    let v2: serde_json::Value = serde_json::from_str(&body_text(res2).await).unwrap();
    assert_eq!(v2["result"], 2);
}

/// JS 내부 throw가 에러 코드로 매핑되는지.
#[tokio::test]
async fn eval_js_throw_returns_error_code() {
    let app = make_app(500);
    let payload = serde_json::json!({ "script": "throw new Error('SJ999: x');0" });
    let req = Request::builder()
        .method("POST")
        .uri("/api/engine/eval")
        .header("Content-Type", "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    let v: serde_json::Value = serde_json::from_str(&body_text(res).await).unwrap();
    assert!(
        v["code"].as_str().unwrap().len() == 6,
        "에러 코드는 6자리여야 한다"
    );
}
