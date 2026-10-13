//! Phase 12: DomainPipeline + ERP/Blog/Shop 스키마 테스트.
//!
//! - DomainPipeline build + handle_request
//! - ERP/Blog/Shop DataStore save/get
//! - 도메인별 훅 발동

use meta_server::domain::DomainPipeline;
use meta_server::engine::{Engine, ThrottleConfig};
use meta_server::meta::redis::InMemoryPubSub;
use meta_server::meta::store::{
    BlogArticle, DataStore, ErpPurchaseOrder, MemoryStore, ShopOrder,
};
use meta_server::observability::{EventQueueObserver, Observability};
use std::sync::Arc;

fn test_engine() -> Engine {
    Engine::new(ThrottleConfig {
        memory_limit: 16 * 1024 * 1024,
        max_stack: 0,
        cpu_timeout_ms: 1500,
    })
    .unwrap()
}

fn test_store() -> Arc<dyn DataStore> {
    Arc::new(MemoryStore::new())
}

fn test_redis() -> Arc<dyn meta_server::meta::redis::RedisLikeStore> {
    Arc::new(InMemoryPubSub::new())
}

fn test_obs() -> Arc<Observability> {
    let (obs, _rx) = EventQueueObserver::new(500, 256);
    Arc::new(Observability::new(Arc::new(obs), 500))
}

/// ERP 파이프라인 빌드 + 요청 처리.
#[tokio::test]
async fn erp_pipeline_build_and_request() {
    let engine = test_engine();
    let store = test_store();
    let redis = test_redis();
    let obs = test_obs();

    let base_src = r#"
        var api = {};
        api.before_process = function(payload) {
            if (!payload.workflow_id) {
                throw new Error(JSON.stringify({code: "EJ1201", message: "워크플로우 ID 필수"}));
            }
            return payload;
        };
        api.after_process = function(event) { return event; };
    "#;
    let ext_src = r#"
        api.before_process = function(payload) {
            var validated = Base.before_process(payload);
            if (payload.quantity <= 0) {
                throw new Error(JSON.stringify({code: "EJ1202", message: "수량 1 이상"}));
            }
            return validated;
        };
    "#;

    let pipeline = DomainPipeline::build(
        &engine, store, redis, obs,
        "erp", "base.erp", "ext.erp_po",
        base_src, ext_src,
    ).await.unwrap();

    // 유효한 발주서
    let payload = serde_json::json!({
        "workflow_id": "WF001",
        "quantity": 10,
        "supplier": "Test Corp"
    });
    let (status, axum::Json(body)) = pipeline.handle_request(payload).await.unwrap();
    assert_eq!(status, axum::http::StatusCode::CREATED);
    assert_eq!(body.code, "OK");
    assert_eq!(body.domain, "erp");

    // 수량 0 → EJ1202
    let payload = serde_json::json!({
        "workflow_id": "WF001",
        "quantity": 0,
        "supplier": "Test Corp"
    });
    let result = pipeline.handle_request(payload).await;
    assert!(result.is_err(), "수량 0 → 에러");
}

/// Blog 파이프라인 빌드 + 요청 처리.
#[tokio::test]
async fn blog_pipeline_build_and_request() {
    let engine = test_engine();
    let store = test_store();
    let redis = test_redis();
    let obs = test_obs();

    let base_src = r#"
        var api = {};
        api.before_publish = function(payload) {
            if (!payload.category) {
                throw new Error(JSON.stringify({code: "BJ1201", message: "카테고리 필수"}));
            }
            if (!payload.title) {
                throw new Error(JSON.stringify({code: "BJ1202", message: "제목 필수"}));
            }
            return payload;
        };
        api.after_publish = function(event) {
            event.slug = event.title ? event.title.toLowerCase().replace(/[^a-z0-9]+/g, "-") : "";
            return event;
        };
    "#;
    let ext_src = r#"
        api.before_publish = function(payload) {
            var validated = Base.before_publish(payload);
            if (typeof validated.languages === "undefined") {
                validated.languages = ["general"];
            }
            return validated;
        };
    "#;

    let pipeline = DomainPipeline::build(
        &engine, store, redis, obs,
        "blog", "base.blog", "ext.blog_tech",
        base_src, ext_src,
    ).await.unwrap();

    // 유효한 블로그 글
    let payload = serde_json::json!({
        "title": "Rust SDK Guide",
        "category": "tech",
        "body": "How to use rust_calculate_base_tax..."
    });
    let (status, _) = pipeline.handle_request(payload).await.unwrap();
    assert_eq!(status, axum::http::StatusCode::CREATED);

    // 카테고리 없음 → BJ1201
    let payload = serde_json::json!({
        "title": "Test",
        "body": "test"
    });
    let result = pipeline.handle_request(payload).await;
    assert!(result.is_err(), "카테고리 없음 → 에러");
}

/// Shop 파이프라인 빌드 + SDK 함수 호출.
#[tokio::test]
async fn shop_pipeline_with_sdk() {
    let engine = test_engine();
    let store = test_store();
    let redis = test_redis();
    let obs = test_obs();

    let base_src = r#"
        var api = {};
        api.before_order = function(payload) {
            if (payload.price < 0) {
                throw new Error(JSON.stringify({code: "SJ1401", message: "가격 0 이상"}));
            }
            if (!payload.product_id) {
                throw new Error(JSON.stringify({code: "SJ1402", message: "상품 ID 필수"}));
            }
            var taxed = rust_calculate_base_tax(payload.price, 0.10);
            if (isNaN(taxed)) {
                throw new Error(JSON.stringify({code: "SJ1401", message: "세전 계산 실패"}));
            }
            payload.taxed_price = taxed;
            return payload;
        };
        api.after_order = function(event) { return event; };
    "#;
    let ext_src = r#"
        api.before_order = function(payload) {
            var validated = Base.before_order(payload);
            if (payload.quantity <= 0) {
                throw new Error(JSON.stringify({code: "SJ1403", message: "수량 1 이상"}));
            }
            return validated;
        };
    "#;

    let pipeline = DomainPipeline::build(
        &engine, store, redis, obs,
        "shop", "base.shop", "ext.shop_electronics",
        base_src, ext_src,
    ).await.unwrap();

    // 유효한 주문 (SDK 함수 호출 포함)
    let payload = serde_json::json!({
        "product_id": "PROD001",
        "price": 10000,
        "quantity": 2
    });
    let (status, _) = pipeline.handle_request(payload).await.unwrap();
    assert_eq!(status, axum::http::StatusCode::CREATED);

    // 음수 가격 → SJ1401
    let payload = serde_json::json!({
        "product_id": "PROD001",
        "price": -100,
        "quantity": 1
    });
    let result = pipeline.handle_request(payload).await;
    assert!(result.is_err(), "음수 가격 → 에러");
}

/// DataStore ERP/Blog/Shop save/get 테스트.
#[tokio::test]
async fn datastore_erp_blog_shop() {
    let store: Arc<dyn DataStore> = Arc::new(MemoryStore::new());

    // ERP
    let erp = ErpPurchaseOrder {
        id: 0,
        workflow_id: "WF001".into(),
        quantity: 10,
        supplier: "Test".into(),
        status: "pending".into(),
        extra: serde_json::Value::Null,
        created_at: 0,
        tenant_id: "default".into(),
    };
    let id = store.save_erp_purchase_order(&erp).await.unwrap();
    assert!(id > 0);
    let fetched = store.get_erp_purchase_order(id).await.unwrap().unwrap();
    assert_eq!(fetched.workflow_id, "WF001");
    assert_eq!(fetched.quantity, 10);

    // Blog
    let blog = BlogArticle {
        id: 0,
        title: "Test".into(),
        body: "Body".into(),
        category: "tech".into(),
        slug: "test".into(),
        extra: serde_json::Value::Null,
        created_at: 0,
        tenant_id: "default".into(),
        template_id: String::new(), template_version: 0, sync_policy: String::from("frozen"),
    };
    let id = store.save_blog_article(&blog).await.unwrap();
    assert!(id > 0);
    let fetched = store.get_blog_article(id).await.unwrap().unwrap();
    assert_eq!(fetched.title, "Test");
    assert_eq!(fetched.category, "tech");

    // Shop
    let shop = ShopOrder {
        id: 0,
        product_id: "P001".into(),
        price: 10000.0,
        quantity: 2,
        status: "pending".into(),
        extra: serde_json::Value::Null,
        created_at: 0,
        tenant_id: "default".into(),
    };
    let id = store.save_shop_order(&shop).await.unwrap();
    assert!(id > 0);
    let fetched = store.get_shop_order(id).await.unwrap().unwrap();
    assert_eq!(fetched.product_id, "P001");
    assert!((fetched.price - 10000.0).abs() < f64::EPSILON);
    assert_eq!(fetched.quantity, 2);
}
