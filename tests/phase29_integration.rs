//! Phase 29 통합 테스트: E2E 파이프라인 + DB 검증 + Cache + Input validation.
//!
//! MemoryStore 기반 E2E 시나리오.

use std::sync::Arc;

use meta_server::cache::L1Cache;
use meta_server::domain::DomainPipeline;
use meta_server::engine::{Engine, ModuleLoader, ThrottleConfig};
use meta_server::meta::redis::InMemoryPubSub;
use meta_server::meta::store::{DataStore, MetaModule, MemoryStore};
use meta_server::observability::Observability;
use meta_server::pipeline::{BoardPipeline, CreatePostRequest, ModuleLoaderRef};

fn tc() -> ThrottleConfig {
    ThrottleConfig {
        memory_limit: 32 * 1024 * 1024,
        max_stack: 0,
        cpu_timeout_ms: 2000,
    }
}

fn board_base() -> MetaModule {
    MetaModule {
        module_id: "base.board".into(),
        kind: "base".into(),
        base_module_id: None,
        script_source: r#"
            api.before_create = function(p){
                if(!p.title){ throw new Error(JSON.stringify({code:"SJ1201",message:"제목 필수"})); }
                return p;
            };
            api.after_save = function(e){ e.saved = true; return e; };
        "#
        .into(),
        content_hash: "h1".into(),
        version: 1,
        enabled: true,
    }
}

fn board_ext() -> MetaModule {
    MetaModule {
        module_id: "ext.notice".into(),
        kind: "extension".into(),
        base_module_id: Some("base.board".into()),
        script_source: r#"
            api.before_create = function(p){
                var v = Base.before_create(p);
                if(typeof v.pinned === "undefined"){ v.pinned = false; }
                return v;
            };
        "#
        .into(),
        content_hash: "h2".into(),
        version: 1,
        enabled: true,
    }
}

struct BoardFixture {
    pipeline: BoardPipeline,
    store: Arc<MemoryStore>,
}

async fn board_fixture() -> BoardFixture {
    let store = Arc::new(MemoryStore::new());
    store.upsert_module(&board_base()).await.unwrap();
    store.upsert_module(&board_ext()).await.unwrap();
    let store_box: Arc<dyn DataStore> = store.clone();

    let engine = Engine::new(tc()).unwrap();
    let cache = Arc::new(L1Cache::new(Arc::clone(&store_box)));
    let loader = ModuleLoader::new(engine.clone(), tc());

    let pipeline = BoardPipeline {
        loader: ModuleLoaderRef::new(loader),
        cache,
        store: store_box,
        observability: Arc::new(Observability::noop()),
    };
    BoardFixture { pipeline, store }
}

fn erp_base() -> MetaModule {
    MetaModule {
        module_id: "base.erp".into(),
        kind: "base".into(),
        base_module_id: None,
        script_source: r#"
            api.before_process = function(p){
                if(!p.workflow_id){ throw new Error(JSON.stringify({code:"EJ1201",message:"워크플로우 ID 필수"})); }
                return p;
            };
            api.after_process = function(e){ return e; };
        "#
        .into(),
        content_hash: "e1".into(),
        version: 1,
        enabled: true,
    }
}

fn erp_ext() -> MetaModule {
    MetaModule {
        module_id: "ext.erp_po".into(),
        kind: "extension".into(),
        base_module_id: Some("base.erp".into()),
        script_source: r#"
            api.before_process = function(p){
                var v = Base.before_process(p);
                if(p.quantity <= 0){ throw new Error(JSON.stringify({code:"EJ1202",message:"수량 1 이상"})); }
                if(!p.supplier){ throw new Error(JSON.stringify({code:"EJ1203",message:"공급자 필수"})); }
                return v;
            };
        "#
        .into(),
        content_hash: "e2".into(),
        version: 1,
        enabled: true,
    }
}

struct ErpFixture {
    pipeline: DomainPipeline,
    store: Arc<MemoryStore>,
}

async fn erp_fixture() -> ErpFixture {
    let store = Arc::new(MemoryStore::new());
    let store_box: Arc<dyn DataStore> = store.clone();
    let redis = Arc::new(InMemoryPubSub::new());

    let engine = Engine::new(tc()).unwrap();
    // DomainPipeline::build가 base/ext 모듈을 자체 upsert
    let pipeline = DomainPipeline::build(
        &engine,
        store_box,
        redis,
        Arc::new(Observability::noop()),
        "erp",
        "base.erp",
        "ext.erp_po",
        &erp_base().script_source,
        &erp_ext().script_source,
    )
    .await
    .unwrap();
    ErpFixture { pipeline, store }
}

/// D29-1: E2E Board Create + DB 검증.
#[tokio::test]
async fn e2e_board_create_and_db_verify() {
    let f = board_fixture().await;
    let req = CreatePostRequest {
        title: "E2E 글".into(),
        body: "본문".into(),
        extra: serde_json::json!({}),
    };
    let (status, resp) = f.pipeline.handle_create(req).await.unwrap();
    assert_eq!(status, axum::http::StatusCode::CREATED);
    let post_id = resp.0.id;
    assert!(post_id > 0);

    // DB에서 저장 확인
    let saved = f.store.get_board_post(post_id).await.unwrap();
    assert!(saved.is_some(), "DB에 저장된 글이 있어야 함");
    let post = saved.unwrap();
    assert_eq!(post.title, "E2E 글");

    // L1 cache에 적재 확인
    let cache_len = f.pipeline.cache.len().await;
    assert!(cache_len >= 2, "L1에 base+ext 모듈 적재 확인");
}

/// D29-2: E2E Domain (ERP) Create + DB 검증.
#[tokio::test]
async fn e2e_erp_create_and_db_verify() {
    let f = erp_fixture().await;
    let payload = serde_json::json!({
        "workflow_id": "wf-e2e",
        "supplier": "Acme",
        "quantity": 10,
        "amount": 1000
    });
    let (status, resp) = f.pipeline.handle_request(payload).await.unwrap();
    assert_eq!(status, axum::http::StatusCode::CREATED);
    let order_id = resp.0.id;

    let saved = f.store.get_erp_purchase_order(order_id).await.unwrap();
    assert!(saved.is_some(), "DB에 저장된 발주서가 있어야 함");
    let order = saved.unwrap();
    assert_eq!(order.supplier, "Acme");
}

/// D29-3: Input Validation — 빈 title → CJ1201 (422).
#[tokio::test]
async fn input_validation_empty_title() {
    let f = board_fixture().await;
    let req = CreatePostRequest {
        title: "".into(),
        body: "본문".into(),
        extra: serde_json::json!({}),
    };
    let res = f.pipeline.handle_create(req).await;
    let (status, body) = res.unwrap_err();
    assert_eq!(status, axum::http::StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        body.0.get("code").and_then(|c| c.as_str()),
        Some("CJ1201")
    );
}

/// D29-3: Input Validation — 음수 quantity (ERP) → CJ1201.
#[tokio::test]
async fn input_validation_negative_quantity_erp() {
    let f = erp_fixture().await;
    let payload = serde_json::json!({
        "workflow_id": "wf-1",
        "supplier": "Acme",
        "quantity": -5,
        "amount": 1000
    });
    let res = f.pipeline.handle_request(payload).await;
    let (status, body) = res.unwrap_err();
    assert_eq!(status, axum::http::StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        body.0.get("code").and_then(|c| c.as_str()),
        Some("CJ1201")
    );
}

/// D29-4: Cache Invalidation → L1 제거 → 재로드.
#[tokio::test]
async fn cache_invalidation_cycle() {
    let f = board_fixture().await;
    // base+ext 로드 (L1 적재)
    let _ = f.pipeline.cache.get_module("base.board").await;
    let _ = f.pipeline.cache.get_module("ext.notice").await;
    assert_eq!(f.pipeline.cache.len().await, 2, "L1에 2개 모듈 적재");

    // 무효화 → L1에서 제거
    f.pipeline.cache.invalidate("base.board").await;
    let len_after_invalidate = f.pipeline.cache.len().await;
    assert_eq!(len_after_invalidate, 1, "무효화 후 L1에 1개 남음");

    // 재로드 → L1에 다시 적재
    let m = f.pipeline.cache.get_module("base.board").await;
    assert!(m.is_some(), "재로드 성공");
    assert_eq!(f.pipeline.cache.len().await, 2, "재로드 후 L1에 2개");
}

/// D36-1: 게시판 태그 CRUD (create + list by post).
#[tokio::test]
async fn board_tag_crud() {
    use meta_server::meta::store::{BoardTag, DataStore};
    let store = meta_server::meta::store::MemoryStore::new();
    let tag = BoardTag {
        id: 0,
        board_id: "board".into(),
        post_id: 10,
        tag: "공지".into(),
        created_at: 0,
        tenant_id: "default".into(),
    };
    let id = store.create_board_tag(&tag).await.unwrap();
    assert!(id > 0, "태그 생성 성공");
    let tags = store.list_board_tags_by_post(10).await.unwrap();
    assert_eq!(tags.len(), 1, "게시글별 태그 1개");
    assert_eq!(tags[0].tag, "공지");
    let none = store.list_board_tags_by_post(99).await.unwrap();
    assert!(none.is_empty(), "다른 게시글은 태그 없음");
}

/// D36-2: 블로그 댓글 CRUD (create + list by article).
#[tokio::test]
async fn blog_comment_crud() {
    use meta_server::meta::store::{BlogComment, DataStore};
    let store = meta_server::meta::store::MemoryStore::new();
    let c1 = BlogComment {
        id: 0,
        article_id: 5,
        author: "user1".into(),
        body: "좋은 글입니다".into(),
        parent_id: None,
        created_at: 0,
        tenant_id: "default".into(),
    };
    let c2 = BlogComment {
        id: 0,
        article_id: 5,
        author: "user2".into(),
        body: "답변드립니다".into(),
        parent_id: None,
        created_at: 0,
        tenant_id: "default".into(),
    };
    let id1 = store.create_blog_comment(&c1).await.unwrap();
    let _id2 = store.create_blog_comment(&c2).await.unwrap();
    assert!(id1 > 0, "댓글 생성 성공");
    let comments = store.list_blog_comments_by_article(5).await.unwrap();
    assert_eq!(comments.len(), 2, "글별 댓글 2개");
    let none = store.list_blog_comments_by_article(99).await.unwrap();
    assert!(none.is_empty(), "다른 글은 댓글 없음");
}

/// D37-1: tenant 격리 — 동일 tenant만 조회, 다른 tenant 접근 불가.
#[tokio::test]
async fn tenant_isolation_scoped_list() {    use meta_server::meta::store::{BoardPost, DataStore};
    let store = meta_server::meta::store::MemoryStore::new();
    let p1 = BoardPost {
        id: 0, title: "a".into(), body: "".into(),
        extra: serde_json::Value::Null, created_at: 0, tenant_id: "acme".into(),
        template_id: String::new(), template_version: 0, sync_policy: String::from("frozen"),
    };
    let p2 = BoardPost {
        id: 0, title: "b".into(), body: "".into(),
        extra: serde_json::Value::Null, created_at: 0, tenant_id: "globex".into(),
        template_id: String::new(), template_version: 0, sync_policy: String::from("frozen"),
    };
    let _ = store.save_board_post(&p1).await;
    let _ = store.save_board_post(&p2).await;
    let acme = store.list_board_posts_for_tenant("acme").await.unwrap();
    assert_eq!(acme.len(), 1, "acme만 1개");
    assert_eq!(acme[0].title, "a");
    let globex = store.list_board_posts_for_tenant("globex").await.unwrap();
    assert_eq!(globex.len(), 1, "globex만 1개");
    assert_eq!(globex[0].title, "b");
    let none = store.list_board_posts_for_tenant("unknown").await.unwrap();
    assert!(none.is_empty(), "없는 tenant은 0개");
}

/// D38-1: Shop 상품 + 카트 CRUD (create + list).
#[tokio::test]
async fn shop_product_and_cart_crud() {
    use meta_server::meta::store::{DataStore, ShopProduct, ShopCart};
    let store = meta_server::meta::store::MemoryStore::new();

    let product = ShopProduct {
        id: 0,
        name: "무선 마우스".into(),
        price: 35000.0,
        stock: 50,
        category: "accessory".into(),
        slug: "wireless-mouse".into(),
        extra: serde_json::Value::Null,
        created_at: 0,
        tenant_id: "default".into(),
        template_id: String::new(), template_version: 0, sync_policy: String::from("frozen"),
    };
    let pid = store.save_shop_product(&product).await.unwrap();
    assert!(pid > 0, "상품 생성 성공");
    let fetched = store.get_shop_product(pid).await.unwrap().unwrap();
    assert_eq!(fetched.name, "무선 마우스");
    assert_eq!(fetched.price, 35000.0);

    let by_slug = store.get_shop_product_by_slug("wireless-mouse").await.unwrap().unwrap();
    assert_eq!(by_slug.id, pid, "슬러그로 조회");

    let products = store.list_shop_products_for_tenant("default").await.unwrap();
    assert_eq!(products.len(), 1, "default tenant 상품 1개");

    let cart = ShopCart {
        id: 0,
        customer: "user1".into(),
        product_id: pid,
        quantity: 2,
        extra: serde_json::Value::Null,
        created_at: 0,
        tenant_id: "default".into(),
    };
    let _cid = store.save_shop_cart(&cart).await.unwrap();
    let carts = store.list_shop_carts_by_customer("user1").await.unwrap();
    assert_eq!(carts.len(), 1, "고객별 카트 1개");
    assert_eq!(carts[0].product_id, pid);
    assert_eq!(carts[0].quantity, 2);

    let none = store.list_shop_carts_by_customer("user99").await.unwrap();
    assert!(none.is_empty(), "다른 고객은 카트 없음");
}
