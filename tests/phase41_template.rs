//! Phase 41 테스트: 계층형 템플릿 시스템 (데이터 모델 + 트리 결합 + 파일 시드).
//!
//! DataStore 템플릿 연동 (MemoryStore) + 파일 시드 멱등성 + 트리 순환 참조 방지를 검증한다.

use std::sync::Arc;

use meta_server::meta::store::{
    DataStore, MetaTemplate, MemoryStore, seed_templates_from_files,
};

fn sample_template(id: &str, parent: &str, ttype: &str, domain: &str) -> MetaTemplate {
    MetaTemplate {
        template_id: id.to_string(),
        tenant_id: "default".to_string(),
        parent_id: parent.to_string(),
        name: id.to_string(),
        ttype: ttype.to_string(),
        domain: domain.to_string(),
        payload: serde_json::json!({"fields": [], "layout": []}),
        source: "db".to_string(),
        file_path: String::new(),
        version: 1,
        enabled: true,
    }
}

/// 템플릿 저장 후 조회 (버전 증분).
#[tokio::test]
async fn upsert_and_get_template() {
    let store = Arc::new(MemoryStore::new());
    let t = sample_template("page.home", "", "page", "homepage");
    store.upsert_template(&t).await.unwrap();

    let got = store.get_template("page.home", "default").await.unwrap().unwrap();
    assert_eq!(got.template_id, "page.home");
    assert_eq!(got.ttype, "page");

    // 재저장 시 버전 증분 (2).
    store.upsert_template(&t).await.unwrap();
    let got2 = store.get_template("page.home", "default").await.unwrap().unwrap();
    assert_eq!(got2.version, 2);
}

/// 조건별 목록 (domain/parent 필터).
#[tokio::test]
async fn list_templates_with_filters() {
    let store = Arc::new(MemoryStore::new());
    store.upsert_template(&sample_template("page.home", "", "page", "homepage")).await.unwrap();
    store.upsert_template(&sample_template("page.blog", "", "page", "blog")).await.unwrap();
    store
        .upsert_template(&sample_template("body.blog", "page.blog", "body", "blog"))
        .await
        .unwrap();

    // domain=blog 필터 → 2개.
    let blog = store.list_templates("default", "blog", "").await.unwrap();
    assert_eq!(blog.len(), 2);

    // parent=page.blog → 1개 (body.blog).
    let children = store.list_templates("default", "", "page.blog").await.unwrap();
    assert_eq!(children.len(), 1);
    assert_eq!(children[0].template_id, "body.blog");
}

/// 트리 결합 (site → page → body/menu).
#[tokio::test]
async fn build_template_tree() {
    let store = Arc::new(MemoryStore::new());
    store
        .upsert_template(&sample_template("site.shop", "", "site", "shop"))
        .await
        .unwrap();
    store
        .upsert_template(&sample_template("page.home", "site.shop", "page", "homepage"))
        .await
        .unwrap();
    store
        .upsert_template(&sample_template("menu.main", "page.home", "menu", ""))
        .await
        .unwrap();
    store
        .upsert_template(&sample_template("body.hero", "page.home", "body", ""))
        .await
        .unwrap();

    let tree = store.build_template_tree("site.shop", "default").await.unwrap().unwrap();
    assert_eq!(tree.template.template_id, "site.shop");
    assert_eq!(tree.children.len(), 1); // page.home
    assert_eq!(tree.children[0].template.template_id, "page.home");
    assert_eq!(tree.children[0].children.len(), 2); // menu.main, body.hero
}

/// 트리 미존재 시 None.
#[tokio::test]
async fn build_template_tree_missing() {
    let store = Arc::new(MemoryStore::new());
    let tree = store.build_template_tree("nonexistent", "default").await.unwrap();
    assert!(tree.is_none());
}

/// 순환 참조 방지 (A→B→A).
#[tokio::test]
async fn build_template_tree_cycle_safe() {
    let store = Arc::new(MemoryStore::new());
    store
        .upsert_template(&sample_template("a", "", "body", ""))
        .await
        .unwrap();
    store
        .upsert_template(&sample_template("b", "a", "body", ""))
        .await
        .unwrap();
    // a → b → a 순환 (b의 parent를 a로, a의 parent를 b로 설정).
    let mut a = sample_template("a", "", "body", "");
    a.parent_id = "b".to_string();
    store.upsert_template(&a).await.unwrap();
    let mut b = sample_template("b", "a", "body", "");
    b.parent_id = "a".to_string();
    store.upsert_template(&b).await.unwrap();

    // 루트 a의 트리는 무한 루프 없이 terminates.
    let tree = store.build_template_tree("a", "default").await.unwrap();
    assert!(tree.is_some());
}

/// 파일 시드 멱등성 (같은 내용 재시드 안 됨).
#[tokio::test]
async fn seed_templates_idempotent() {
    // templates/ 디렉토리가 존재하는 전제 (CWD가 프로젝트 루트).
    let store = Arc::new(MemoryStore::new());
    let first = seed_templates_from_files(store.as_ref()).await.unwrap();
    let second = seed_templates_from_files(store.as_ref()).await.unwrap();
    // 두 번째 시드는 멱등 (기존 파일 템플릿 skip).
    assert!(second <= first, "멱등성 위반: first={first}, second={second}");
}

// ── Phase 41 Step 2: 인스턴스 바인딩/개선 전파 ──

fn board_post_for_binding(id: i64, tenant: &str) -> meta_server::meta::store::BoardPost {
    meta_server::meta::store::BoardPost {
        id,
        title: "t".into(),
        body: "b".into(),
        extra: serde_json::Value::Null,
        created_at: 0,
        tenant_id: tenant.into(),
        template_id: String::new(),
        template_version: 0,
        sync_policy: "frozen".into(),
    }
}

/// 템플릿 바인딩: template_id/version/sync_policy + payload 기록.
#[tokio::test]
async fn apply_template_to_board_post_records_binding() {
    let store = Arc::new(MemoryStore::new());
    let id = store.save_board_post(&board_post_for_binding(0, "default")).await.unwrap();

    store
        .apply_template_to_board_post(id, "body.blog_post", 3, r#"{"title":"x"}"#, "live")
        .await
        .unwrap();

    let got = store.get_board_post(id).await.unwrap().unwrap();
    assert_eq!(got.template_id, "body.blog_post");
    assert_eq!(got.template_version, 3);
    assert_eq!(got.sync_policy, "live");
    assert_eq!(got.extra["title"], "x");
}

/// 개선 전파: sync_policy='live' 인스턴스만 payload 재적용.
#[tokio::test]
async fn propagate_only_affects_live_instances() {
    let store = Arc::new(MemoryStore::new());
    let a = store.save_board_post(&board_post_for_binding(0, "default")).await.unwrap();
    let b = store.save_board_post(&board_post_for_binding(0, "default")).await.unwrap();

    // 둘 다 바인딩: 하나는 live, 하나는 frozen.
    store.apply_template_to_board_post(a, "tpl", 1, r#"{"v":1}"#, "live").await.unwrap();
    store.apply_template_to_board_post(b, "tpl", 1, r#"{"v":1}"#, "frozen").await.unwrap();

    // 전파 → live(a)만 영향.
    let count = store.propagate_template_to_board_posts("default", "tpl", r#"{"v":2}"#).await.unwrap();
    assert_eq!(count, 1, "live 인스턴스 1개만 전파되어야 함");

    let ga = store.get_board_post(a).await.unwrap().unwrap();
    let gb = store.get_board_post(b).await.unwrap().unwrap();
    assert_eq!(ga.extra["v"], 2, "live 인스턴스는 재적용");
    assert_eq!(gb.extra["v"], 1, "frozen 인스턴스는 그대로");
}

/// 버전 증분 (bump): 활성 버전 +1, 기존 인스턴스 스냅샷 보존.
#[tokio::test]
async fn bump_template_version_increments() {
    let store = Arc::new(MemoryStore::new());
    let mut t = sample_template("body.blog_post", "site.shop", "body", "blog");
    t.payload = serde_json::json!({"fields": [{"key":"title","default":"hi"}], "layout":[]});
    store.upsert_template(&t).await.unwrap();

    let v1 = store.bump_template_version("body.blog_post", "default").await.unwrap();
    let v2 = store.bump_template_version("body.blog_post", "default").await.unwrap();
    assert_eq!(v1, 2); // 첫 증분: version 1 → 2
    assert_eq!(v2, 3); // 두 번째: 2 → 3

    // 활성 버전이 3.
    let active = store.get_template("body.blog_post", "default").await.unwrap().unwrap();
    assert_eq!(active.version, 3);
}

/// 사이트 템플릿 프로비저닝: body 노드를 대상 테넌트로 복사.
#[tokio::test]
async fn apply_site_template_copies_body_nodes() {
    let store = Arc::new(MemoryStore::new());
    // default 테넌트에 site 트리 생성: site.shop → page.home → body.blog_post.
    let mut site = sample_template("site.shop", "", "site", "shop");
    site.tenant_id = "default".into();
    store.upsert_template(&site).await.unwrap();
    let mut page = sample_template("page.home", "site.shop", "page", "homepage");
    page.tenant_id = "default".into();
    store.upsert_template(&page).await.unwrap();
    let mut body = sample_template("body.blog_post", "page.home", "body", "blog");
    body.tenant_id = "default".into();
    store.upsert_template(&body).await.unwrap();

    // acme 테넌트로 프로비저닝 → body 노드 1개 복사.
    let count = store.apply_site_template("acme", "site.shop").await.unwrap();
    assert_eq!(count, 1, "body 노드 1개만 복사");

    // acme 테넌트에 body.blog_post가 존재.
    let got = store.get_template("body.blog_post", "acme").await.unwrap();
    assert!(got.is_some());
    assert_eq!(got.unwrap().tenant_id, "acme");
}

// ── Phase 42 P2: Blog/Shop 스태킹 ──

fn blog_article_for_binding(id: i64, tenant: &str) -> meta_server::meta::store::BlogArticle {
    meta_server::meta::store::BlogArticle {
        id,
        title: "t".into(),
        body: "b".into(),
        category: "cat".into(),
        slug: "".into(),
        extra: serde_json::Value::Null,
        created_at: 0,
        tenant_id: tenant.into(),
        template_id: String::new(),
        template_version: 0,
        sync_policy: "frozen".into(),
    }
}

fn shop_product_for_binding(id: i64, tenant: &str) -> meta_server::meta::store::ShopProduct {
    meta_server::meta::store::ShopProduct {
        id,
        name: "n".into(),
        price: 0.0,
        stock: 0,
        category: "cat".into(),
        slug: "".into(),
        extra: serde_json::Value::Null,
        created_at: 0,
        tenant_id: tenant.into(),
        template_id: String::new(),
        template_version: 0,
        sync_policy: "frozen".into(),
    }
}

/// 블로그 템플릿 바인딩: template_id/version/sync_policy + payload 기록.
#[tokio::test]
async fn apply_template_to_blog_article_records_binding() {
    let store = Arc::new(MemoryStore::new());
    let id = store.save_blog_article(&blog_article_for_binding(0, "default")).await.unwrap();

    store
        .apply_template_to_blog_article(id, "body.blog", 2, r#"{"title":"hi"}"#, "live")
        .await
        .unwrap();

    let got = store.get_blog_article(id).await.unwrap().unwrap();
    assert_eq!(got.template_id, "body.blog");
    assert_eq!(got.template_version, 2);
    assert_eq!(got.sync_policy, "live");
    assert_eq!(got.extra["title"], "hi");
}

/// 쇼핑몰 템플릿 바인딩: template_id/version/sync_policy + payload 기록.
#[tokio::test]
async fn apply_template_to_shop_product_records_binding() {
    let store = Arc::new(MemoryStore::new());
    let id = store.save_shop_product(&shop_product_for_binding(0, "default")).await.unwrap();

    store
        .apply_template_to_shop_product(id, "body.product", 1, r#"{"name":"x"}"#, "frozen")
        .await
        .unwrap();

    let got = store.get_shop_product(id).await.unwrap().unwrap();
    assert_eq!(got.template_id, "body.product");
    assert_eq!(got.template_version, 1);
    assert_eq!(got.sync_policy, "frozen");
    assert_eq!(got.extra["name"], "x");
}

/// 블로그 개선 전파: sync_policy='live' 인스턴스만 payload 재적용.
#[tokio::test]
async fn propagate_only_affects_live_instances_blog() {
    let store = Arc::new(MemoryStore::new());
    let a = store.save_blog_article(&blog_article_for_binding(0, "default")).await.unwrap();
    let b = store.save_blog_article(&blog_article_for_binding(0, "default")).await.unwrap();

    store.apply_template_to_blog_article(a, "tpl", 1, r#"{"v":1}"#, "live").await.unwrap();
    store.apply_template_to_blog_article(b, "tpl", 1, r#"{"v":1}"#, "frozen").await.unwrap();

    let count = store.propagate_template_to_blog_articles("default", "tpl", r#"{"v":2}"#).await.unwrap();
    assert_eq!(count, 1);

    let ga = store.get_blog_article(a).await.unwrap().unwrap();
    let gb = store.get_blog_article(b).await.unwrap().unwrap();
    assert_eq!(ga.extra["v"], 2);
    assert_eq!(gb.extra["v"], 1);
}

/// 쇼핑몰 개선 전파: sync_policy='live' 인스턴스만 payload 재적용.
#[tokio::test]
async fn propagate_only_affects_live_instances_shop() {
    let store = Arc::new(MemoryStore::new());
    let a = store.save_shop_product(&shop_product_for_binding(0, "default")).await.unwrap();
    let b = store.save_shop_product(&shop_product_for_binding(0, "default")).await.unwrap();

    store.apply_template_to_shop_product(a, "tpl", 1, r#"{"v":1}"#, "live").await.unwrap();
    store.apply_template_to_shop_product(b, "tpl", 1, r#"{"v":1}"#, "frozen").await.unwrap();

    let count = store.propagate_template_to_shop_products("default", "tpl", r#"{"v":2}"#).await.unwrap();
    assert_eq!(count, 1);

    let ga = store.get_shop_product(a).await.unwrap().unwrap();
    let gb = store.get_shop_product(b).await.unwrap().unwrap();
    assert_eq!(ga.extra["v"], 2);
    assert_eq!(gb.extra["v"], 1);
}

/// 3개 도메인 동시 전파: board+blog+shop 합계.
#[tokio::test]
async fn propagate_all_domains_count() {
    let store = Arc::new(MemoryStore::new());
    // board 1개 live, blog 1개 live, shop 1개 live.
    let b_id = store.save_board_post(&board_post_for_binding(0, "default")).await.unwrap();
    let bl_id = store.save_blog_article(&blog_article_for_binding(0, "default")).await.unwrap();
    let s_id = store.save_shop_product(&shop_product_for_binding(0, "default")).await.unwrap();

    store.apply_template_to_board_post(b_id, "tpl", 1, r#"{"v":1}"#, "live").await.unwrap();
    store.apply_template_to_blog_article(bl_id, "tpl", 1, r#"{"v":1}"#, "live").await.unwrap();
    store.apply_template_to_shop_product(s_id, "tpl", 1, r#"{"v":1}"#, "live").await.unwrap();

    let total = store.propagate_template_to_all_domains("default", "tpl", r#"{"v":3}"#).await.unwrap();
    assert_eq!(total, 3, "board+blog+shop = 3");

    // 각 도메인 extra 확인.
    let gb = store.get_board_post(b_id).await.unwrap().unwrap();
    let gbl = store.get_blog_article(bl_id).await.unwrap().unwrap();
    let gs = store.get_shop_product(s_id).await.unwrap().unwrap();
    assert_eq!(gb.extra["v"], 3);
    assert_eq!(gbl.extra["v"], 3);
    assert_eq!(gs.extra["v"], 3);
}
