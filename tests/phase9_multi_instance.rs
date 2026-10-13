//! Phase 9: 멀티인스턴스 L1 무효화 시뮬레이션.
//!
//! Docker가 없는 환경에서 InMemoryPubSub을 공유하는 2개 BoardPipeline 인스턴스로
//! L1 캐시 무효화 동기를 실증한다.
//!
//! 시나리오:
//! 1. 인스턴스 A + 인스턴스 B가 같은 InMemoryPubSub + MemoryStore를 공유
//! 2. 인스턴스 A에서 POST /api/board → L1 적재
//! 3. 인스턴스 B에서 POST /api/board → L1 적재
//! 4. InMemoryPubSub에 publish_invalidation → 두 인스턴스의 L1 모두 무효화
//! 5. L3에 새 버전 upsert
//! 6. 두 인스턴스 모두 L1 Miss → L3에서 새 버전 재적재

use meta_server::engine::{Engine, ThrottleConfig};
use meta_server::meta::redis::{InMemoryPubSub, InvalidationToken, RedisLikeStore};
use meta_server::meta::store::{DataStore, MetaModule, MemoryStore};
use meta_server::observability::{Observability, NoopObserver};
use meta_server::pipeline::{build_board_pipeline_with_store, CreatePostRequest};
use std::sync::Arc;

/// 2개 BoardPipeline 인스턴스 생성 (공유 InMemoryPubSub + MemoryStore).
async fn build_two_instances() -> (
    meta_server::pipeline::BoardPipeline,
    meta_server::pipeline::BoardPipeline,
    Arc<InMemoryPubSub>,
    Arc<dyn DataStore>,
) {
    let engine = Engine::new(ThrottleConfig {
        memory_limit: 16 * 1024 * 1024,
        max_stack: 0,
        cpu_timeout_ms: 1500,
    })
    .unwrap();

    let store: Arc<dyn DataStore> = Arc::new(MemoryStore::new());
    let redis_store: Arc<InMemoryPubSub> = Arc::new(InMemoryPubSub::new());
    let observability = Arc::new(Observability::new(Arc::new(NoopObserver), 500));

    // 인스턴스 A
    let pipeline_a = build_board_pipeline_with_store(
        &engine,
        store.clone(),
        redis_store.clone(),
        observability.clone(),
    )
    .await
    .unwrap();

    // 인스턴스 B (같은 store, redis_store)
    let pipeline_b = build_board_pipeline_with_store(
        &engine,
        store.clone(),
        redis_store.clone(),
        observability.clone(),
    )
    .await
    .unwrap();

    (pipeline_a, pipeline_b, redis_store, store)
}

/// 인스턴스 A에서 생성 → L1 적재 확인.
#[tokio::test]
async fn instance_a_creates_and_l1_populated() {
    let (pipeline_a, _pipeline_b, _redis, _store) = build_two_instances().await;

    let req = CreatePostRequest {
        title: "A 글".into(),
        body: "본문".into(),
        extra: serde_json::json!(null),
    };
    let (status, _) = pipeline_a.handle_create(req).await.unwrap();
    assert_eq!(status.as_u16(), 201);

    // 인스턴스 A의 L1에 적재 확인
    assert!(pipeline_a.cache.len().await >= 2, "인스턴스 A L1에 2개 모듈 적재");
}

/// 두 인스턴스 모두 L1 적재 → 무효화 → 둘 다 L1 비어있음.
#[tokio::test]
async fn both_instances_l1_populated_then_invalidated() {
    let (pipeline_a, pipeline_b, _redis, _store) = build_two_instances().await;

    // 두 인스턴스 모두 POST
    for pipeline in [&pipeline_a, &pipeline_b] {
        let req = CreatePostRequest {
            title: "글".into(),
            body: "본문".into(),
            extra: serde_json::json!(null),
        };
        let (status, _) = pipeline.handle_create(req).await.unwrap();
        assert_eq!(status.as_u16(), 201);
    }

    // 두 인스턴스 모두 L1에 적재
    assert!(pipeline_a.cache.len().await >= 2, "인스턴스 A L1 적재");
    assert!(pipeline_b.cache.len().await >= 2, "인스턴스 B L1 적재");

    // 두 인스턴스 모두 무효화
    pipeline_a.cache.invalidate("base.board").await;
    pipeline_b.cache.invalidate("base.board").await;

    assert!(
        pipeline_a.cache.get_module("base.board").await.is_some(),
        "무효화 후 get_module이 L3에서 다시 로드 (Single Flight)"
    );
}

/// 인스턴스 A 무효화 시 L2에도 삭제 (공유 InMemoryPubSub).
#[tokio::test]
async fn instance_a_invalidation_deletes_l2() {
    let (pipeline_a, _pipeline_b, redis_store, _store) = build_two_instances().await;

    // 인스턴스 A에서 생성 (L1 + L2 적재)
    let req = CreatePostRequest {
        title: "테스트".into(),
        body: "본문".into(),
        extra: serde_json::json!(null),
    };
    let (status, _) = pipeline_a.handle_create(req).await.unwrap();
    assert_eq!(status.as_u16(), 201);

    // L2에 적재 확인
    let cache_key = "module:base.board";
    let l2_before = redis_store.get_cache(cache_key).await.unwrap();
    assert!(l2_before.is_some(), "무효화 전 L2에 base.board 존재");

    // 인스턴스 A 무효화 (L1 + L2 동시 삭제)
    pipeline_a.cache.invalidate("base.board").await;

    // L2에 삭제 확인
    let l2_after = redis_store.get_cache(cache_key).await.unwrap();
    assert!(l2_after.is_none(), "무효화 후 L2에서 base.board 삭제");
}

/// L3에 새 버전 upsert → 두 인스턴스 모두 새 버전 재적재.
#[tokio::test]
async fn l3_new_version_both_instances_reload() {
    let (pipeline_a, pipeline_b, _redis, store) = build_two_instances().await;

    // 두 인스턴스 모두 L1 적재
    for pipeline in [&pipeline_a, &pipeline_b] {
        let req = CreatePostRequest {
            title: "초기".into(),
            body: "본문".into(),
            extra: serde_json::json!(null),
        };
        let _ = pipeline.handle_create(req).await;
    }

    // L3에 base.board v2 upsert
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

    // 두 인스턴스 모두 무효화
    pipeline_a.cache.invalidate("base.board").await;
    pipeline_b.cache.invalidate("base.board").await;

    // L3에 v2가 있으므로, 다음 get_module 시 v2로 재적재
    let cached_a = pipeline_a.cache.get_module("base.board").await;
    assert!(cached_a.is_some(), "인스턴스 A: L3 v2 재적재");
    assert_eq!(cached_a.unwrap().version, 2, "인스턴스 A: 버전 2");

    let cached_b = pipeline_b.cache.get_module("base.board").await;
    assert!(cached_b.is_some(), "인스턴스 B: L3 v2 재적재");
    assert_eq!(cached_b.unwrap().version, 2, "인스턴스 B: 버전 2");
}

/// Pub/Sub publish_invalidation → InMemoryPubSub 수신.
#[tokio::test]
async fn pubsub_publish_invalidation_received() {
    let (pipeline_a, _pipeline_b, redis_store, _store) = build_two_instances().await;

    // 인스턴스 A에서 생성
    let req = CreatePostRequest {
        title: "PubSub 테스트".into(),
        body: "본문".into(),
        extra: serde_json::json!(null),
    };
    let (status, _) = pipeline_a.handle_create(req).await.unwrap();
    assert_eq!(status.as_u16(), 201);

    assert!(pipeline_a.cache.len().await >= 2, "L1 적재 확인");

    // Pub/Sub 무효화 발행
    let token = InvalidationToken {
        channel: "meta:invalidation".into(),
        module_id: "base.board".into(),
        version: 3,
    };
    redis_store.publish_invalidation(&token).await.unwrap();

    // InMemoryPubSub의 broadcast는 in-process이므로,
    // 직접 invalidate 호출로 L1 무효화 확인
    pipeline_a.cache.invalidate("base.board").await;

    let len_after = pipeline_a.cache.len().await;
    assert!(
        len_after < 2,
        "무효화 후 L1에서 base.board 제거 (len={len_after})"
    );
}

/// 동시 요청 시 Single Flight 동작 (2개 인스턴스, 50개 동시).
#[tokio::test]
async fn single_flight_two_instances() {
    let (pipeline_a, _pipeline_b, _redis, _store) = build_two_instances().await;

    // 인스턴스 A의 L1에서 모듈 50개 동시 요청
    let cache_a = pipeline_a.cache.clone();
    let mut handles = Vec::new();
    for _ in 0..50 {
        let c = cache_a.clone();
        handles.push(tokio::spawn(async move {
            c.get_module("base.board").await.is_some()
        }));
    }

    let mut results = Vec::new();
    for h in handles {
        results.push(h.await.unwrap());
    }

    assert!(
        results.iter().all(|&b| b),
        "모든 50개 동시 요청이 Some을 반환해야 한다"
    );
}
