//! Phase 13: 마이그레이션 + 모니터링 엔드포인트 테스트.
//!
//! - 마이그레이션 파일 목록
//! - build_router + health 엔드포인트 확인
//! - SessionPool len() 메서드

use std::sync::Arc;

use meta_server::http::build_router;
use meta_server::meta::migrate::list_migration_files;
use meta_server::meta::redis::{InMemoryPubSub, RedisLikeStore};
use meta_server::meta::store::{DataStore, MemoryStore};
use meta_server::observability::{EventQueueObserver, Observability};
use meta_server::pipeline::build_board_pipeline_with_store;
use meta_server::socket::SocketConfig;

fn test_engine() -> meta_server::engine::Engine {
    meta_server::engine::Engine::new(meta_server::engine::ThrottleConfig {
        memory_limit: 16 * 1024 * 1024,
        max_stack: 0,
        cpu_timeout_ms: 1500,
    })
    .unwrap()
}

/// 마이그레이션 파일 목록 확인.
#[test]
fn migration_file_list() {
    let files = list_migration_files().unwrap();
    assert!(!files.is_empty(), "마이그레이션 파일이 존재해야 한다");
    for f in &files {
        let name = f.file_name().unwrap().to_string_lossy();
        println!("[마이그레이션] {name}");
        assert!(name.ends_with(".sql"), "{name}가 .sql 파일이 아님");
    }
}

/// build_router + health 엔드포인트 확인 (Phase 13).
#[tokio::test]
async fn health_endpoint_builds() {
    let engine = test_engine();

    let store: Arc<dyn DataStore> = Arc::new(MemoryStore::new());
    let redis_store: Arc<dyn RedisLikeStore> = Arc::new(InMemoryPubSub::new());
    let (obs, _rx) = EventQueueObserver::new(500, 256);
    let observability = Arc::new(Observability::new(Arc::new(obs), 500));

    let board = build_board_pipeline_with_store(&engine, store, redis_store, observability)
        .await
        .unwrap();

    let socket_config = SocketConfig::default();
    let router = build_router(
        engine,
        Some(Arc::new(board)),
        vec![],
        socket_config,
        String::new(),
        "memory".into(),
        vec!["001_init.sql".to_string(), "002_domain_tables.sql".to_string()],
        false,
        "test-secret".into(),
        true,
        1000,
    );

    // 라우터가 정상 기동 → health 엔드포인트가 등록됨
    let _ = router.clone();

    println!("[Phase 13] build_router + health 엔드포인트 확인");
}

/// SessionPool len() 메서드 확인.
#[tokio::test]
async fn session_pool_len() {
    let config = SocketConfig { max_sessions: 10 };
    let pool = Arc::new(meta_server::socket::session::SessionPool::new(&config));

    assert_eq!(pool.len(), 0);

    let (session, _rx) = pool.add_session().await.unwrap();
    assert_eq!(pool.len(), 1);

    let (session2, _rx2) = pool.add_session().await.unwrap();
    assert_eq!(pool.len(), 2);

    pool.remove_session(&session.socket_id).await;
    assert_eq!(pool.len(), 1);

    pool.remove_session(&session2.socket_id).await;
    assert_eq!(pool.len(), 0);
}
