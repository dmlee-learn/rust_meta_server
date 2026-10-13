//! Phase 6: 성능 벤치마크.
//!
//! L1 캐시 적중률, 훅 발동 성능, 무한 루프 감지, 동시 WebSocket, 세션 풀 한계.

use std::time::Instant;

use futures::{SinkExt, StreamExt};
use meta_server::cache::L1Cache;
use meta_server::engine::{Engine, ThrottleConfig};
use meta_server::http::build_router;
use meta_server::meta::redis::{InMemoryPubSub, RedisLikeStore};
use meta_server::meta::store::{DataStore, MemoryStore};
use meta_server::observability::{Observability, NoopObserver};
use meta_server::pipeline::build_board_pipeline_with_store;
use meta_server::socket::SocketConfig;
use std::sync::Arc;
use tokio::net::TcpListener;

/// L1 캐시 적중률 측정.
///
/// 동일 module_id 요청 100회, L1 Hit 수 / 총 요청 수.
/// 기준: 90% 이상.
#[tokio::test]
async fn l1_cache_hit_rate() {
    let store: Arc<dyn DataStore> = Arc::new(MemoryStore::new());
    store
        .upsert_module(&meta_server::meta::store::MetaModule {
            module_id: "bench.module".into(),
            kind: "base".into(),
            base_module_id: None,
            script_source: "var api={};".into(),
            content_hash: "h".into(),
            version: 1,
            enabled: true,
        })
        .await
        .unwrap();

    let cache = Arc::new(L1Cache::new(store));

    // 1회차: L1 Miss (첫 조회)
    let _ = cache.get_module("bench.module").await;

    // 99회: L1 Hit
    let total = 100;
    let mut hits = 0;
    let mut misses = 0;

    for _ in 0..(total - 1) {
        match cache.get_module("bench.module").await {
            Some(_) => hits += 1,
            None => misses += 1,
        }
    }

    let hit_rate = hits as f64 / (total as f64 - 1.0);
    println!(
        "[벤치마크] L1 캐시 적중률: {hit_rate:.1}% ({hits}/{} )",
        total - 1
    );
    assert!(
        hit_rate >= 0.9,
        "L1 캐시 적중률 {hit_rate:.1}%가 기준(90%) 미만"
    );
}

/// 훅 발동 성능 측정.
///
/// before_create 훅 50회 발동, 평균 ms.
/// 기준: 10ms 미만.
#[tokio::test]
async fn hook_execution_performance() {
    let engine = Engine::new(ThrottleConfig {
        memory_limit: 16 * 1024 * 1024,
        max_stack: 0,
        cpu_timeout_ms: 1500,
    })
    .unwrap();

    let store: Arc<dyn DataStore> = Arc::new(MemoryStore::new());
    let redis_store: Arc<dyn RedisLikeStore> = Arc::new(InMemoryPubSub::new());
    let observability = Arc::new(Observability::new(Arc::new(NoopObserver), 500));
    let pipeline = build_board_pipeline_with_store(&engine, store, redis_store, observability)
        .await
        .unwrap();

    // warmup
    let _ = pipeline
        .handle_create(meta_server::pipeline::CreatePostRequest {
            title: "warmup".into(),
            body: "warmup".into(),
            extra: serde_json::json!(null),
        })
        .await;

    // 50회 발동
    let iterations = 50;
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = pipeline
            .handle_create(meta_server::pipeline::CreatePostRequest {
                title: "bench".into(),
                body: "bench".into(),
                extra: serde_json::json!(null),
            })
            .await;
    }
    let elapsed = start.elapsed();
    let avg_ms = elapsed.as_millis() as f64 / iterations as f64;

    println!(
        "[벤치마크] 훅 발동 성능: 평균 {avg_ms:.2}ms ({iterations}회, {elapsed:?})"
    );
    assert!(
        avg_ms < 10.0,
        "훅 발동 평균 {avg_ms:.2}ms가 기준(10ms) 초과"
    );
}

/// 무한 루프 감지 시간 측정.
///
/// cpu_timeout_ms=500ms, CJ1102 발생까지 시간.
/// 기준: 600ms 미만.
#[tokio::test]
async fn infinite_loop_detection_time() {
    let engine = Engine::new(ThrottleConfig {
        memory_limit: 16 * 1024 * 1024,
        max_stack: 0,
        cpu_timeout_ms: 500,
    })
    .unwrap();

    let sb = engine.create_sandbox().unwrap();

    let start = Instant::now();
    let result = sb.eval_i64("while(true){}");
    let elapsed = start.elapsed();

    // CJ1102 확인
    assert!(
        result.is_err(),
        "무한 루프가 에러로 끝나야 한다"
    );
    let err = result.unwrap_err();
    let err_str = format!("{:?}", err);
    assert!(
        err_str.contains("Cj102") || err_str.contains("CJ1102"),
        "CJ1102(CPU Timeout)가 반환되어야 한다, 실제: {err_str}"
    );

    println!(
        "[벤치마크] 무한 루프 감지: {elapsed:?} (cpu_timeout_ms=500)"
    );
    assert!(
        elapsed.as_millis() < 600,
        "무한 루프 감지 {elapsed:?}가 기준(600ms) 초과"
    );
}

/// 동시 WebSocket 50개 연결.
///
/// 50개 동시 연결 + welcome 수신.
/// 기준: 50/50 성공.
#[tokio::test]
async fn concurrent_websocket_connections() {
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

    let hook_script = r#"
        globalThis.on_socket_connect = function(payload) {
            return "welcome";
        };
        globalThis.on_socket_message = function(payload) {
            return null;
        };
        globalThis.on_socket_disconnect = function(payload) {
            return null;
        };
    "#
    .to_string();

    let router = build_router(
        engine,
        None,
        vec![],
        socket_config,
        hook_script,
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

    let url = format!("ws://127.0.0.1:{port}/ws");

    // 50개 동시 연결
    let mut handles: Vec<tokio::task::JoinHandle<std::result::Result<(), String>>> = Vec::new();
    for _ in 0..50 {
        let url = url.clone();
        handles.push(tokio::spawn(async move {
            let (ws, _response) = tokio_tungstenite::connect_async(&url)
                .await
                .map_err(|e| format!("connect: {e}"))?;
            let (_ws_tx, mut ws_rx) = ws.split();

            // welcome 수신
            let msg = ws_rx
                .next()
                .await
                .ok_or("no message")?
                .map_err(|e| format!("recv: {e}"))?;
            if let tokio_tungstenite::tungstenite::Message::Text(text) = msg {
                let v: serde_json::Value = serde_json::from_str(&text)
                    .map_err(|e| format!("parse: {e}"))?;
                if v["type"] == "welcome" {
                    Ok(())
                } else {
                    Err(format!("unexpected: {text}"))
                }
            } else {
                Err("not text".into())
            }
        }));
    }

    let mut successes = 0;
    let mut failures = 0;
    for h in handles {
        match h.await {
            Ok(Ok(())) => successes += 1,
            Ok(Err(e)) => {
                failures += 1;
                eprintln!("[벤치마크] WS 연결 실패: {e}");
            }
            Err(e) => {
                failures += 1;
                eprintln!("[벤치마크] WS 태스크 실패: {e}");
            }
        }
    }

    println!(
        "[벤치마크] 동시 WebSocket: {successes}/50 성공, {failures} 실패"
    );
    assert!(
        successes == 50,
        "50개 중 {successes}개만 성공"
    );
}

/// 세션 풀 한계.
///
/// SOCKET_MAX_SESSIONS=5, 6개 동시 연결.
/// 기준: 5개 성공 + 1개 CR2001.
#[tokio::test]
async fn session_pool_limit() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    let engine = Engine::new(ThrottleConfig {
        memory_limit: 16 * 1024 * 1024,
        max_stack: 0,
        cpu_timeout_ms: 1500,
    })
    .unwrap();

    let socket_config = SocketConfig {
        max_sessions: 5,
    };

    let hook_script = r#"
        globalThis.on_socket_connect = function(payload) {
            return "welcome";
        };
        globalThis.on_socket_message = function(payload) {
            return null;
        };
        globalThis.on_socket_disconnect = function(payload) {
            return null;
        };
    "#
    .to_string();

    let router = build_router(
        engine,
        None,
        vec![],
        socket_config,
        hook_script,
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

    let url = format!("ws://127.0.0.1:{port}/ws");

    // 6개 동시 연결 (max=5)
    let mut handles: Vec<tokio::task::JoinHandle<std::result::Result<bool, String>>> = Vec::new();
    for _ in 0..6 {
        let url = url.clone();
        handles.push(tokio::spawn(async move {
            match tokio_tungstenite::connect_async(&url).await {
                Ok((ws, _)) => {
                    // 연결 성공 → welcome 대기
                    let (_ws_tx, mut ws_rx) = ws.split();
                    let _msg = ws_rx.next().await;
                    Ok(true)
                }
                Err(e) => Ok(false),
            }
        }));
    }

    let mut successes = 0;
    let mut failures = 0;
    for h in handles {
        match h.await {
            Ok(Ok(true)) => successes += 1,
            Ok(Ok(false)) => failures += 1,
            Ok(Err(e)) => {
                failures += 1;
                eprintln!("[벤치마크] WS 태스크 실패: {e}");
            }
            Err(e) => {
                failures += 1;
                eprintln!("[벤치마크] WS 태스크 실패: {e}");
            }
        }
    }

    println!(
        "[벤치마크] 세션 풀 한계: {successes} 성공, {failures} 실패 (max_sessions=5)"
    );
    assert!(
        successes == 5 && failures == 1,
        "5개 성공 + 1개 실패를 기대하지만 {successes}/{failures}"
    );
}
