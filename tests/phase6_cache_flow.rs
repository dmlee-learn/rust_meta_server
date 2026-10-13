//! Phase 6: L1 무효화 → 재적재 시나리오 테스트.
//!
//! 다층 캐시 흐름 실증:
//! 1. L1 적재 (POST /api/board)
//! 2. L1 무효화 (invalidate)
//! 3. L3에 새 버전 upsert
//! 4. L1 재적재 (새 버전)

use meta_server::cache::L1Cache;
use meta_server::engine::{Engine, ThrottleConfig};
use meta_server::meta::redis::{InMemoryPubSub, RedisLikeStore};
use meta_server::meta::store::{DataStore, MetaModule, MemoryStore};
use meta_server::observability::{Observability, NoopObserver};
use meta_server::pipeline::build_board_pipeline_with_store;
use std::sync::Arc;

async fn build_test_pipeline() -> (
    meta_server::pipeline::BoardPipeline,
    Arc<L1Cache>,
    Arc<dyn DataStore>,
) {
    let engine = Engine::new(ThrottleConfig {
        memory_limit: 16 * 1024 * 1024,
        max_stack: 0,
        cpu_timeout_ms: 1500,
    })
    .unwrap();

    let store: Arc<dyn DataStore> = Arc::new(MemoryStore::new());
    let redis_store: Arc<dyn RedisLikeStore> = Arc::new(InMemoryPubSub::new());
    let observability = Arc::new(Observability::new(Arc::new(NoopObserver), 500));

    let pipeline = build_board_pipeline_with_store(&engine, store.clone(), redis_store, observability)
        .await
        .unwrap();

    let cache = pipeline.cache.clone();
    (pipeline, cache, store)
}

/// L1 적재 → 무효화 → L3 새 버전 upsert → L1 재적재.
#[tokio::test]
async fn l1_invalidation_and_reload() {
    let (pipeline, cache, store) = build_test_pipeline().await;

    // 1. L1 적재 (POST /api/board 처리 시 L1 적재)
    let req = meta_server::pipeline::CreatePostRequest {
        title: "테스트 글".into(),
        body: "본문".into(),
        extra: serde_json::json!(null),
    };
    let _ = pipeline.handle_create(req).await;

    // L1에 base.board + ext.notice 적재 확인
    assert!(cache.len().await >= 2, "L1에 적어도 2개 모듈이 적재되어야 한다");

    // 2. L1 무효화
    cache.invalidate("base.board").await;
    let len_after = cache.len().await;
    assert_eq!(
        len_after, 1,
        "무효화 후 L1에 base.board만 제거되어야 한다 (len=1), 실제: {len_after}"
    );

    // 3. L3에 base.board v2 upsert
    let v2_module = MetaModule {
        module_id: "base.board".into(),
        kind: "base".into(),
        base_module_id: None,
        script_source: "var api = {};\napi.before_create = function(p) { return p; };\n".into(),
        content_hash: "base_h2".into(),
        version: 2,
        enabled: true,
    };
    store.upsert_module(&v2_module).await.unwrap();

    // 4. L1 재적재 (get_module → L1 Miss → L3 v2 조회)
    let cached = cache.get_module("base.board").await;
    assert!(cached.is_some(), "재적재 후 L1에서 base.board가 Some이어야 한다");
    let cached = cached.unwrap();
    assert_eq!(cached.version, 2, "재적재된 버전은 2여야 한다");
    assert_eq!(cached.module.version, 2, "모듈 버전은 2여야 한다");
}

/// 두 모듈 모두 무효화 → 재적재.
#[tokio::test]
async fn l1_invalidation_multiple_modules() {
    let (pipeline, cache, store) = build_test_pipeline().await;

    // 1. L1 적재
    let req = meta_server::pipeline::CreatePostRequest {
        title: "테스트".into(),
        body: "본문".into(),
        extra: serde_json::json!(null),
    };
    let _ = pipeline.handle_create(req).await;

    // 2. 두 모듈 모두 무효화
    cache.invalidate("base.board").await;
    cache.invalidate("ext.notice").await;
    let len_after = cache.len().await;
    assert_eq!(
        len_after, 0,
        "두 모듈 모두 무효화 후 L1에 0개만 있어야 한다, 실제: {len_after}"
    );

    // 3. L3에 두 모듈 v2 upsert
    let base_v2 = MetaModule {
        module_id: "base.board".into(),
        kind: "base".into(),
        base_module_id: None,
        script_source: "var api = {};\napi.before_create = function(p) { return p; };\n".into(),
        content_hash: "base_h2".into(),
        version: 2,
        enabled: true,
    };
    let ext_v2 = MetaModule {
        module_id: "ext.notice".into(),
        kind: "extension".into(),
        base_module_id: Some("base.board".into()),
        script_source: "var api = {};\napi.before_create = function(p) { return Base.before_create(p); };\n".into(),
        content_hash: "ext_h2".into(),
        version: 2,
        enabled: true,
    };
    store.upsert_module(&base_v2).await.unwrap();
    store.upsert_module(&ext_v2).await.unwrap();

    // 4. L1 재적재
    let base = cache.get_module("base.board").await;
    assert!(base.is_some(), "재적재 후 base.board가 Some이어야 한다");
    assert_eq!(base.unwrap().version, 2);

    let ext = cache.get_module("ext.notice").await;
    assert!(ext.is_some(), "재적재 후 ext.notice가 Some이어야 한다");
    assert_eq!(ext.unwrap().version, 2);
}
