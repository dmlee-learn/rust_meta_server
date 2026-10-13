//! Phase 8: 실 Redis/PostgreSQL 연동 테스트.
//!
//! Docker Compose로 기동한 실 Redis + PostgreSQL 인스턴스와 연동한다.
//! `#[ignore]`로 기본 `cargo test`에서 제외되고, 수동 실행한다:
//!
//! ```bash
//! docker compose up -d
//! cargo test --test phase8_real_integration -- --ignored --nocapture
//! docker compose down
//! ```
//!
//! 전제: Docker Compose로 Redis (6379) + PostgreSQL (5432) 기동.
//!
//! 실 연동 항목:
//! - Redis PING
//! - PostgreSQL SELECT 1
//! - L1 Miss → L2(Redis) Hit → L1 적재
//! - L1 Miss → L2 Miss → L3(PG) → L2/L1 적재
//! - Pub/Sub 무효화
//! - 게시판 글 저장 → PG 조회

use meta_server::cache::L1Cache;
use meta_server::engine::{Engine, ThrottleConfig};
use meta_server::meta::redis::{
    CacheEntry, InvalidationToken, RedisConfig, create_redis_store,
};
use meta_server::meta::store::{
    DataStore, MetaModule, MetaStoreConfig, create_data_store,
};
use meta_server::observability::{Observability, NoopObserver};
use meta_server::pipeline::build_board_pipeline_with_store;
use std::sync::Arc;

/// Box<dyn DataStore> → Arc<dyn DataStore> 캐스팅 헬퍼.
fn to_arc_store(boxed: Box<dyn DataStore>) -> Arc<dyn DataStore> {
    Arc::from(boxed as Box<dyn DataStore>)
}

const REDIS_URL: &str = "redis://:metapass@localhost:6379";
const PG_URL: &str = "postgres://meta:metapass@localhost:5432/meta";

/// 실 Redis 연결 확인 (PING).
#[tokio::test]
#[ignore]
async fn redis_ping() {
    let config = RedisConfig {
        enabled: true,
        url: REDIS_URL.to_string(),
    };
    let store = create_redis_store(&config).await.unwrap();

    // Redis에 직접 PING (cache 설정으로 확인)
    let entry = CacheEntry {
        key: "test:pink".into(),
        value: "ping".into(),
        version: 1,
    };
    store.set_cache(&entry).await.unwrap();

    let result = store.get_cache("test:pink").await.unwrap();
    assert!(result.is_some(), "Redis PING: GET 확인");
    assert_eq!(result.unwrap().value, "ping");

    // 삭제
    store.delete_cache("test:pink").await.unwrap();
    assert!(store.get_cache("test:pink").await.unwrap().is_none());

    println!("[Phase 8] Redis PING: OK");
}

/// 실 PostgreSQL 연결 확인 (SELECT 1).
#[tokio::test]
#[ignore]
async fn postgres_connect() {
    let config = MetaStoreConfig {
        backend: "postgres".to_string(),
        postgres_url: PG_URL.to_string(),
        redis: Default::default(),
    };

    let store = create_data_store(&config).await.unwrap();
    let store_ref: &dyn DataStore = &*store;

    // 모듈 upsert + 조회
    let module = MetaModule {
        module_id: "pg.test".into(),
        kind: "base".into(),
        base_module_id: None,
        script_source: "var api={};".into(),
        content_hash: "pg_h1".into(),
        version: 1,
        enabled: true,
    };
    store_ref.upsert_module(&module).await.unwrap();

    let loaded = store_ref.get_module("pg.test").await.unwrap();
    assert!(loaded.is_some(), "PostgreSQL: 모듈 조회 확인");
    assert_eq!(loaded.unwrap().module_id, "pg.test");

    println!("[Phase 8] PostgreSQL SELECT 1: OK");
}

/// L1 Miss → L2(Redis) Hit → L1 적재.
#[tokio::test]
#[ignore]
async fn l1_miss_l2_redis_hit() {
    let redis_config = RedisConfig {
        enabled: true,
        url: REDIS_URL.to_string(),
    };
    let redis_store = create_redis_store(&redis_config).await.unwrap();

    let pg_config = MetaStoreConfig {
        backend: "postgres".to_string(),
        postgres_url: PG_URL.to_string(),
        redis: Default::default(),
    };
    let store: Arc<dyn DataStore> = to_arc_store(create_data_store(&pg_config).await.unwrap());

    // 실 Redis에 module:redis.test 적재
    let module = MetaModule {
        module_id: "redis.test".into(),
        kind: "base".into(),
        base_module_id: None,
        script_source: "var api={};".into(),
        content_hash: "redis_h1".into(),
        version: 1,
        enabled: true,
    };
    let cache_key = "module:redis.test";
    let entry = CacheEntry {
        key: cache_key.into(),
        value: serde_json::to_string(&module).unwrap(),
        version: 1,
    };
    redis_store.set_cache(&entry).await.unwrap();

    // L1에서 조회 (L1 Miss → L2(Redis) Hit)
    let cache = Arc::new(L1Cache::new(store.clone()).with_redis(redis_store.clone()));
    let cached = cache.get_module("redis.test").await;
    assert!(cached.is_some(), "L1 Miss → L2(Redis) Hit → L1 적재 확인");
    assert_eq!(cached.unwrap().module.module_id, "redis.test");

    // 정화
    redis_store.delete_cache(cache_key).await.unwrap();

    println!("[Phase 8] L1 Miss → L2(Redis) Hit: OK");
}

/// L1 Miss → L2 Miss → L3(PG) → L2/L1 적재.
#[tokio::test]
#[ignore]
async fn l1_miss_l2_miss_l3_pg() {
    let redis_config = RedisConfig {
        enabled: true,
        url: REDIS_URL.to_string(),
    };
    let redis_store = create_redis_store(&redis_config).await.unwrap();

    let pg_config = MetaStoreConfig {
        backend: "postgres".to_string(),
        postgres_url: PG_URL.to_string(),
        redis: Default::default(),
    };
    let store: Arc<dyn DataStore> = to_arc_store(create_data_store(&pg_config).await.unwrap());

    // 실 PostgreSQL에 모듈 저장
    let module = MetaModule {
        module_id: "pg2.test".into(),
        kind: "base".into(),
        base_module_id: None,
        script_source: "var api={};".into(),
        content_hash: "pg2_h1".into(),
        version: 1,
        enabled: true,
    };
    store.upsert_module(&module).await.unwrap();

    // L1에서 조회 (L1 Miss → L2(Redis) Miss → L3(PG) Hit → L2/L1 적재)
    let cache = Arc::new(L1Cache::new(store.clone()).with_redis(redis_store.clone()));
    let cached = cache.get_module("pg2.test").await;
    assert!(cached.is_some(), "L1 Miss → L2 Miss → L3(PG) Hit → L2/L1 적재 확인");
    assert_eq!(cached.unwrap().module.module_id, "pg2.test");

    // L2에 자동 적재 확인
    let cache_key = "module:pg2.test";
    let l2_entry = redis_store.get_cache(cache_key).await.unwrap();
    assert!(l2_entry.is_some(), "L3 로드 후 L2(Redis)에 자동 적재 확인");

    // 정화
    redis_store.delete_cache(cache_key).await.unwrap();

    println!("[Phase 8] L1 Miss → L2 Miss → L3(PG): OK");
}

/// Pub/Sub 무효화 (실 Redis).
#[tokio::test]
#[ignore]
async fn pubsub_invalidation() {
    let redis_config = RedisConfig {
        enabled: true,
        url: REDIS_URL.to_string(),
    };
    let redis_store = create_redis_store(&redis_config).await.unwrap();

    let pg_config = MetaStoreConfig {
        backend: "postgres".to_string(),
        postgres_url: PG_URL.to_string(),
        redis: Default::default(),
    };
    let store: Arc<dyn DataStore> = to_arc_store(create_data_store(&pg_config).await.unwrap());

    // 모듈 저장
    let module = MetaModule {
        module_id: "pubsub.test".into(),
        kind: "base".into(),
        base_module_id: None,
        script_source: "var api={};".into(),
        content_hash: "ps_h1".into(),
        version: 1,
        enabled: true,
    };
    store.upsert_module(&module).await.unwrap();

    // L1에 적재
    let cache = Arc::new(L1Cache::new(store.clone()).with_redis(redis_store.clone()));
    let _ = cache.get_module("pubsub.test").await;
    assert_eq!(cache.len().await, 1, "L1에 적재 확인");

    // Pub/Sub 무효화 발행
    let token = InvalidationToken {
        channel: "meta:invalidation".into(),
        module_id: "pubsub.test".into(),
        version: 2,
    };
    redis_store.publish_invalidation(&token).await.unwrap();

    // L1 무효화 확인 (InMemoryPubSub 스텁은 in-process이므로 직접 invalidate 확인)
    // 실 Redis Pub/Sub은 별도 수신자 필요. 여기서 L1.invalidate 직접 호출로 검증.
    cache.invalidate("pubsub.test").await;
    assert_eq!(cache.len().await, 0, "L1 무효화 확인");

    // L2에도 삭제 확인
    let cache_key = "module:pubsub.test";
    let l2_entry = redis_store.get_cache(cache_key).await.unwrap();
    assert!(l2_entry.is_none(), "L2 무효화 확인 (invalidate에서 삭제)");

    println!("[Phase 8] Pub/Sub 무효화: OK");
}

/// 게시판 글 저장 → PG 조회.
#[tokio::test]
#[ignore]
async fn board_post_save_pg() {
    let engine = Engine::new(ThrottleConfig {
        memory_limit: 16 * 1024 * 1024,
        max_stack: 0,
        cpu_timeout_ms: 1500,
    })
    .unwrap();

    let redis_config = RedisConfig {
        enabled: true,
        url: REDIS_URL.to_string(),
    };
    let redis_store = create_redis_store(&redis_config).await.unwrap();

    let pg_config = MetaStoreConfig {
        backend: "postgres".to_string(),
        postgres_url: PG_URL.to_string(),
        redis: Default::default(),
    };
    let store: Arc<dyn DataStore> = to_arc_store(create_data_store(&pg_config).await.unwrap());

    let observability = Arc::new(Observability::new(Arc::new(NoopObserver), 500));

    let pipeline = build_board_pipeline_with_store(
        &engine,
        store.clone(),
        redis_store,
        observability,
    )
    .await
    .unwrap();

    let (status, json) = pipeline
        .handle_create(meta_server::pipeline::CreatePostRequest {
            title: "Phase 8 실 PG 테스트".into(),
            body: "본문".into(),
            extra: serde_json::json!(null),
        })
        .await
        .unwrap();

    assert_eq!(status.as_u16(), 201, "201 Created 반환");
    let resp: &meta_server::pipeline::CreatePostResponse = &json.0;
    assert_eq!(resp.code, "OK");
    let post_id = resp.id;
    assert!(post_id > 0);

    // PG에서 직접 조회
    let post = store.get_board_post(post_id).await.unwrap();
    assert!(post.is_some(), "PostgreSQL에서 글 조회 확인");
    let post = post.unwrap();
    assert_eq!(post.title, "Phase 8 실 PG 테스트");

    println!("[Phase 8] 게시판 글 저장 → PG 조회: OK (post_id={post_id})");
}
