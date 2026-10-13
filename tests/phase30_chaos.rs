//! Phase 30 Chaos Engineering: 장애 주입 + graceful degradation + recovery.
//!
//! Redis(L2) 장애 시 L1/L3 fallback, save 장애 시 에러 매핑 검증.

use std::sync::Arc;

use async_trait::async_trait;

use meta_server::cache::L1Cache;
use meta_server::engine::{Engine, ModuleLoader, ThrottleConfig};
use meta_server::meta::redis::{
    CacheEntry, InvalidationToken, RedisLikeError, RedisLikeStore,
};
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

/// Failing Redis: 모든 캐시 호출이 실패를 반환.
struct FailingRedis;

#[async_trait]
impl RedisLikeStore for FailingRedis {
    async fn ping(&self) -> Result<(), RedisLikeError> {
        Err(RedisLikeError::Connect("Redis down".into()))
    }

    async fn get_cache(
        &self,
        _key: &str,
    ) -> Result<Option<CacheEntry>, RedisLikeError> {
        Err(RedisLikeError::Cache("Redis down".into()))
    }

    async fn set_cache(&self, _entry: &CacheEntry) -> Result<(), RedisLikeError> {
        Err(RedisLikeError::Cache("Redis down".into()))
    }

    async fn delete_cache(&self, _key: &str) -> Result<(), RedisLikeError> {
        Err(RedisLikeError::Cache("Redis down".into()))
    }

    async fn publish_invalidation(
        &self,
        _token: &InvalidationToken,
    ) -> Result<(), RedisLikeError> {
        Err(RedisLikeError::PubSub("Redis down".into()))
    }

    async fn subscribe_invalidation(
        &self,
        _handler: Box<dyn FnMut(InvalidationToken) + Send>,
    ) -> Result<(), RedisLikeError> {
        Err(RedisLikeError::PubSub("Redis down".into()))
    }
}

/// Failing Store: save_board_post가 항상 실패.
struct FailingStore {
    inner: MemoryStore,
}

#[async_trait::async_trait]
impl DataStore for FailingStore {
    async fn upsert_module(
        &self,
        m: &MetaModule,
    ) -> Result<(), meta_server::meta::store::DataStoreError> {
        self.inner.upsert_module(m).await
    }

    async fn get_module(
        &self,
        id: &str,
    ) -> Result<Option<MetaModule>, meta_server::meta::store::DataStoreError> {
        self.inner.get_module(id).await
    }

    async fn list_enabled_modules(
        &self,
    ) -> Result<Vec<MetaModule>, meta_server::meta::store::DataStoreError> {
        self.inner.list_enabled_modules().await
    }

    async fn save_board_post(
        &self,
        _post: &meta_server::meta::store::BoardPost,
    ) -> Result<i64, meta_server::meta::store::DataStoreError> {
        Err(meta_server::meta::store::DataStoreError::DbQuery(
            "DB down".into(),
        ))
    }

    async fn get_board_post(
        &self,
        _id: i64,
    ) -> Result<Option<meta_server::meta::store::BoardPost>, meta_server::meta::store::DataStoreError> {
        Ok(None)
    }

    async fn save_erp_purchase_order(
        &self,
        order: &meta_server::meta::store::ErpPurchaseOrder,
    ) -> Result<i64, meta_server::meta::store::DataStoreError> {
        self.inner.save_erp_purchase_order(order).await
    }

    async fn get_erp_purchase_order(
        &self,
        id: i64,
    ) -> Result<Option<meta_server::meta::store::ErpPurchaseOrder>, meta_server::meta::store::DataStoreError> {
        self.inner.get_erp_purchase_order(id).await
    }

    async fn save_blog_article(
        &self,
        _a: &meta_server::meta::store::BlogArticle,
    ) -> Result<i64, meta_server::meta::store::DataStoreError> {
        Err(meta_server::meta::store::DataStoreError::DbQuery(
            "DB down".into(),
        ))
    }

    async fn get_blog_article(
        &self,
        _id: i64,
    ) -> Result<Option<meta_server::meta::store::BlogArticle>, meta_server::meta::store::DataStoreError> {
        Ok(None)
    }

    async fn save_shop_order(
        &self,
        _o: &meta_server::meta::store::ShopOrder,
    ) -> Result<i64, meta_server::meta::store::DataStoreError> {
        Err(meta_server::meta::store::DataStoreError::DbQuery(
            "DB down".into(),
        ))
    }

    async fn get_shop_order(
        &self,
        _id: i64,
    ) -> Result<Option<meta_server::meta::store::ShopOrder>, meta_server::meta::store::DataStoreError> {
        Ok(None)
    }

    async fn create_board_tag(
        &self,
        _t: &meta_server::meta::store::BoardTag,
    ) -> Result<i64, meta_server::meta::store::DataStoreError> {
        Err(meta_server::meta::store::DataStoreError::DbQuery("DB down".into()))
    }

    async fn list_board_tags_by_post(
        &self,
        _post_id: i64,
    ) -> Result<Vec<meta_server::meta::store::BoardTag>, meta_server::meta::store::DataStoreError> {
        Ok(Vec::new())
    }

    async fn create_blog_comment(
        &self,
        _c: &meta_server::meta::store::BlogComment,
    ) -> Result<i64, meta_server::meta::store::DataStoreError> {
        Err(meta_server::meta::store::DataStoreError::DbQuery("DB down".into()))
    }

    async fn list_blog_comments_by_article(
        &self,
        _article_id: i64,
    ) -> Result<Vec<meta_server::meta::store::BlogComment>, meta_server::meta::store::DataStoreError> {
        Ok(Vec::new())
    }

    async fn list_board_posts_for_tenant(
        &self,
        _tenant_id: &str,
    ) -> Result<Vec<meta_server::meta::store::BoardPost>, meta_server::meta::store::DataStoreError> {
        Ok(Vec::new())
    }

    async fn list_blog_articles_for_tenant(
        &self,
        _tenant_id: &str,
    ) -> Result<Vec<meta_server::meta::store::BlogArticle>, meta_server::meta::store::DataStoreError> {
        Ok(Vec::new())
    }

    async fn list_shop_orders_for_tenant(
        &self,
        _tenant_id: &str,
    ) -> Result<Vec<meta_server::meta::store::ShopOrder>, meta_server::meta::store::DataStoreError> {
        Ok(Vec::new())
    }

    async fn list_erp_orders_for_tenant(
        &self,
        _tenant_id: &str,
    ) -> Result<Vec<meta_server::meta::store::ErpPurchaseOrder>, meta_server::meta::store::DataStoreError> {
        Ok(Vec::new())
    }

    async fn save_shop_product(
        &self,
        _product: &meta_server::meta::store::ShopProduct,
    ) -> Result<i64, meta_server::meta::store::DataStoreError> {
        Ok(1)
    }

    async fn get_shop_product(
        &self,
        _id: i64,
    ) -> Result<Option<meta_server::meta::store::ShopProduct>, meta_server::meta::store::DataStoreError> {
        Ok(None)
    }

    async fn list_shop_products_for_tenant(
        &self,
        _tenant_id: &str,
    ) -> Result<Vec<meta_server::meta::store::ShopProduct>, meta_server::meta::store::DataStoreError> {
        Ok(Vec::new())
    }

    async fn get_shop_product_by_slug(
        &self,
        _slug: &str,
    ) -> Result<Option<meta_server::meta::store::ShopProduct>, meta_server::meta::store::DataStoreError> {
        Ok(None)
    }

    async fn save_shop_cart(
        &self,
        _cart: &meta_server::meta::store::ShopCart,
    ) -> Result<i64, meta_server::meta::store::DataStoreError> {
        Ok(1)
    }

    async fn list_shop_carts_by_customer(
        &self,
        _customer: &str,
    ) -> Result<Vec<meta_server::meta::store::ShopCart>, meta_server::meta::store::DataStoreError> {
        Ok(Vec::new())
    }

    async fn get_template(
        &self, id: &str, tenant: &str,
    ) -> Result<Option<meta_server::meta::store::MetaTemplate>, meta_server::meta::store::DataStoreError> {
        self.inner.get_template(id, tenant).await
    }

    async fn list_templates(
        &self, tenant: &str, domain: &str, parent: &str,
    ) -> Result<Vec<meta_server::meta::store::MetaTemplate>, meta_server::meta::store::DataStoreError> {
        self.inner.list_templates(tenant, domain, parent).await
    }

    async fn list_template_children(
        &self, parent: &str, tenant: &str,
    ) -> Result<Vec<meta_server::meta::store::MetaTemplate>, meta_server::meta::store::DataStoreError> {
        self.inner.list_template_children(parent, tenant).await
    }

    async fn upsert_template(
        &self, t: &meta_server::meta::store::MetaTemplate,
    ) -> Result<(), meta_server::meta::store::DataStoreError> {
        self.inner.upsert_template(t).await
    }

    async fn build_template_tree(
        &self, id: &str, tenant: &str,
    ) -> Result<Option<meta_server::meta::store::TemplateTree>, meta_server::meta::store::DataStoreError> {
        self.inner.build_template_tree(id, tenant).await
    }

    async fn apply_template_to_board_post(
        &self,
        post_id: i64,
        template_id: &str,
        template_version: i64,
        payload_fields: &str,
        sync_policy: &str,
    ) -> Result<(), meta_server::meta::store::DataStoreError> {
        self.inner.apply_template_to_board_post(post_id, template_id, template_version, payload_fields, sync_policy).await
    }

    async fn bump_template_version(
        &self, id: &str, tenant: &str,
    ) -> Result<i64, meta_server::meta::store::DataStoreError> {
        self.inner.bump_template_version(id, tenant).await
    }

    async fn propagate_template_to_board_posts(
        &self,
        tenant: &str,
        id: &str,
        payload_fields: &str,
    ) -> Result<i64, meta_server::meta::store::DataStoreError> {
        self.inner.propagate_template_to_board_posts(tenant, id, payload_fields).await
    }

    async fn apply_site_template(
        &self, tenant: &str, site_template_id: &str,
    ) -> Result<i64, meta_server::meta::store::DataStoreError> {
        self.inner.apply_site_template(tenant, site_template_id).await
    }

    // ── Phase 42 P2: Blog/Shop 스태킹 ──

    async fn apply_template_to_blog_article(
        &self,
        article_id: i64,
        template_id: &str,
        template_version: i64,
        payload_fields: &str,
        sync_policy: &str,
    ) -> Result<(), meta_server::meta::store::DataStoreError> {
        self.inner.apply_template_to_blog_article(article_id, template_id, template_version, payload_fields, sync_policy).await
    }

    async fn apply_template_to_shop_product(
        &self,
        product_id: i64,
        template_id: &str,
        template_version: i64,
        payload_fields: &str,
        sync_policy: &str,
    ) -> Result<(), meta_server::meta::store::DataStoreError> {
        self.inner.apply_template_to_shop_product(product_id, template_id, template_version, payload_fields, sync_policy).await
    }

    async fn propagate_template_to_blog_articles(
        &self,
        tenant: &str,
        id: &str,
        payload_fields: &str,
    ) -> Result<i64, meta_server::meta::store::DataStoreError> {
        self.inner.propagate_template_to_blog_articles(tenant, id, payload_fields).await
    }

    async fn propagate_template_to_shop_products(
        &self,
        tenant: &str,
        id: &str,
        payload_fields: &str,
    ) -> Result<i64, meta_server::meta::store::DataStoreError> {
        self.inner.propagate_template_to_shop_products(tenant, id, payload_fields).await
    }

    async fn propagate_template_to_all_domains(
        &self,
        tenant: &str,
        id: &str,
        payload_fields: &str,
    ) -> Result<i64, meta_server::meta::store::DataStoreError> {
        self.inner.propagate_template_to_all_domains(tenant, id, payload_fields).await
    }
}

fn base_board() -> MetaModule {
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

fn ext_notice() -> MetaModule {
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

/// T30-1: Redis(L2) 장애 → L1 miss 시 L3 fallback 로드.
///
/// FailingRedis + MemoryStore: L1 miss → L2 실패 → L3 로드 성공.
#[tokio::test]
async fn redis_down_falls_back_to_l3() {
    let store = MemoryStore::new();
    store.upsert_module(&base_board()).await.unwrap();
    store.upsert_module(&ext_notice()).await.unwrap();
    let store_box: Arc<dyn DataStore> = Arc::new(store);

    let engine = Engine::new(tc()).unwrap();
    // L2(FailingRedis) 연결된 cache
    let cache = Arc::new(
        L1Cache::new(store_box.clone()).with_redis(Arc::new(FailingRedis)),
    );

    // L1 miss → L2(FailingRedis) 실패 → L3 로드
    let m = cache.get_module("base.board").await;
    assert!(m.is_some(), "L2 실패해도 L3에서 로드되어야 함");
    assert_eq!(m.unwrap().module.module_id, "base.board");

    // L1에 적재 확인
    assert_eq!(cache.len().await, 1);
}

/// T30-2: Recovery — invalidate 후 재로드.
#[tokio::test]
async fn recovery_after_invalidation() {
    let store = MemoryStore::new();
    store.upsert_module(&base_board()).await.unwrap();
    store.upsert_module(&ext_notice()).await.unwrap();
    let store_box: Arc<dyn DataStore> = Arc::new(store);

    let cache = Arc::new(L1Cache::new(store_box));

    // 적재
    assert!(cache.get_module("base.board").await.is_some());
    assert_eq!(cache.len().await, 1);

    // 무효화 (재covery trigger)
    cache.invalidate("base.board").await;
    assert_eq!(cache.len().await, 0, "invalidate 후 L1 비어 있음");

    // 재로드 (recovery)
    assert!(cache.get_module("base.board").await.is_some());
    assert_eq!(cache.len().await, 1, "재로드 후 L1 적재");
}

/// T30-3: Save 장애 → 에러 응답 (패닉 아님).
#[tokio::test]
async fn save_failure_returns_error_not_panic() {
    let failing_store = FailingStore {
        inner: MemoryStore::new(),
    };
    // 모듈은 내부를 사용 (모듈 로딩은 성공)
    failing_store
        .inner
        .upsert_module(&base_board())
        .await
        .unwrap();
    failing_store
        .inner
        .upsert_module(&ext_notice())
        .await
        .unwrap();
    let store_box: Arc<dyn DataStore> = Arc::new(failing_store);

    let engine = Engine::new(tc()).unwrap();
    let cache = Arc::new(L1Cache::new(store_box.clone()));
    let loader = ModuleLoader::new(engine.clone(), tc());

    let pipeline = BoardPipeline {
        loader: ModuleLoaderRef::new(loader),
        cache,
        store: store_box,
        observability: Arc::new(Observability::noop()),
    };

    let req = CreatePostRequest {
        title: "Chaos 글".into(),
        body: "본문".into(),
        extra: serde_json::json!({}),
    };
    // save 실패 → 에러 반환 (패닉 아님)
    let res = pipeline.handle_create(req).await;
    assert!(res.is_err(), "save 장애 시 에러 반환");
    let (status, body) = res.unwrap_err();
    assert!(
        status.is_client_error() || status.is_server_error(),
        "4xx/5xx 에러 상태"
    );
    assert!(
        body.0.get("code").and_then(|c| c.as_str()).is_some(),
        "에러 코드 포함"
    );
}
