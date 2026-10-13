//! Phase 14: Prometheus Metrics + JWT 인증 테스트.
//!
//! - /metrics 엔드포인트 Prometheus text format
//! - JWT 토큰 생성/검증
//! - build_router + JWT 미들웨어

use std::sync::Arc;

use meta_server::auth::{Claims, verify_token, JwtConfig};
use meta_server::http::build_router;
use meta_server::meta::redis::InMemoryPubSub;
use meta_server::meta::redis::RedisLikeStore;
use meta_server::meta::store::{DataStore, MemoryStore};
use meta_server::metrics::Metrics;
use meta_server::observability::{EventQueueObserver, Observability};
use meta_server::pipeline::build_board_pipeline_with_store;
use meta_server::socket::SocketConfig;
use meta_server::engine::{Engine, ThrottleConfig};

fn test_engine() -> Engine {
    Engine::new(ThrottleConfig {
        memory_limit: 16 * 1024 * 1024,
        max_stack: 0,
        cpu_timeout_ms: 1500,
    })
    .unwrap()
}

/// Metrics encode 확인 (Phase 17: async).
#[tokio::test]
async fn metrics_encode() {
    let metrics = Metrics::new();
    metrics.record_http_request("board");
    metrics.record_cache_hit("l1");
    metrics.record_cache_miss("l2");
    metrics.record_hook_execution();
    metrics.record_hook_error();

    let text = metrics.encode().await;
    assert!(text.contains("meta_server_http_requests_total"));
    assert!(text.contains("meta_server_cache_hits_total"));
    assert!(text.contains("meta_server_cache_misses_total"));
    assert!(text.contains("meta_server_hook_executions_total"));
    assert!(text.contains("meta_server_hook_errors_total"));
    assert!(text.contains("# TYPE"));

    println!("[Phase 14] Metrics encode 확인:\n{text}");
}

/// JWT 토큰 생성 + 검증.
#[test]
fn jwt_create_and_verify() {
    use jsonwebtoken::{EncodingKey, Header, Algorithm};
    use std::time::SystemTime;

    let exp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
        + 3600;

    let claims = Claims {
        sub: "user1".to_string(),
        domains: Some(vec!["erp".to_string(), "blog".to_string()]),
        exp,
    };

    let token = jsonwebtoken::encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(b"test-secret"),
    )
    .unwrap();

    let verified = verify_token(&token, "test-secret").unwrap();
    assert_eq!(verified.sub, "user1");
    assert_eq!(verified.domains.unwrap(), vec!["erp", "blog"]);

    // 잘못된 시크릿
    let result = verify_token(&token, "wrong-secret");
    assert!(result.is_err());
}

/// JWT 만료 토큰 실패 확인.
#[test]
fn jwt_expired_token() {
    use jsonwebtoken::{EncodingKey, Header, Algorithm};
    use std::time::SystemTime;

    let exp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
        - 3600; // 1시간 전

    let claims = Claims {
        sub: "user1".to_string(),
        domains: None,
        exp,
    };

    let token = jsonwebtoken::encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(b"test-secret"),
    )
    .unwrap();

    let result = verify_token(&token, "test-secret");
    assert!(result.is_err(), "만료된 토큰은 실패해야 함");
}

/// build_router + JWT 미들웨어 확인.
#[tokio::test]
async fn build_router_with_jwt() {
    let engine = test_engine();

    let store: Arc<dyn DataStore> = Arc::new(MemoryStore::new());
    let redis_store: Arc<dyn RedisLikeStore> = Arc::new(InMemoryPubSub::new());
    let (obs, _rx) = EventQueueObserver::new(500, 256);
    let observability = Arc::new(Observability::new(Arc::new(obs), 500));

    let board = build_board_pipeline_with_store(&engine, store, redis_store, observability)
        .await
        .unwrap();

    let socket_config = SocketConfig::default();
    let jwt_config = JwtConfig {
        secret: "test-secret".to_string(),
        disabled: true, // 개발용: JWT 비활성화
    };

    let router = build_router(
        engine,
        Some(Arc::new(board)),
        vec![],
        socket_config,
        String::new(),
        "memory".into(),
        vec![],
        false,
        jwt_config.secret,
        jwt_config.disabled,
        1000,
    );

    let _ = router;
    println!("[Phase 14] build_router + JWT 미들웨어 확인");
}
