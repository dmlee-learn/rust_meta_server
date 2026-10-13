//! `DataStore` 추상층: PostgreSQL(기본) + 메모리 스텁(폴백).
//!
//! AGENTS.md의 3단계 데이터 레이어 중 L3(PostgreSQL)를 담당하며,
//! 인스턴스가 없으면 메모리 스텁으로 폴백해 Phase 2를 실데이터 없이
//! 검증할 수 있게 한다 (Q2-3).
//!
//! 설정: `DATABASE_BACKEND=postgres|memory`. 기본값은 `postgres` (Q1-1).
//! PostgreSQL 연결 실패 시 기동 시 명시적 에러를 반환하며,
//! `DATABASE_BACKEND=memory`로 전환하면 스텁으로 동작.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;
use tokio_postgres::{Client, NoTls};
/// 메타 모듈 레코드 (L3 스키마).
///
/// `scripts/base`와 `scripts/extensions`에 정의된 모듈의 선언적 상태.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MetaModule {
    /// 모듈 고유 ID (예: "base.board", "ext.notice").
    pub module_id: String,
    /// 모듈 종류: "base" 또는 "extension".
    pub kind: String,
    /// 기준 모듈 ID (extension만, base면 null).
    pub base_module_id: Option<String>,
    /// 스크립트 소스 코드 (JS).
    pub script_source: String,
    /// 스크립트 내용 해시 (CJ1101 캐시 적재 시 검증).
    pub content_hash: String,
    /// 최적 버전 (낙관적 버전 바인딩).
    pub version: i32,
    /// 활성화 여부.
    pub enabled: bool,
}

/// DataStore 백엔드 설정.
#[derive(Debug, Clone)]
pub struct MetaStoreConfig {
    /// "postgres" | "memory". 기본값 "postgres" (Q1-1).
    pub backend: String,
    /// PostgreSQL 연결 문자열 (postgres backend일 때).
    pub postgres_url: String,
    /// Redis(L2) 설정. 기본값: 비활성 (InMemoryPubSub 스텁).
    pub redis: crate::meta::redis::RedisConfig,
}

impl Default for MetaStoreConfig {
    fn default() -> Self {
        Self {
            backend: "postgres".to_string(),
            postgres_url: "postgres://user:pass@localhost:5432/meta".to_string(),
            redis: crate::meta::redis::RedisConfig::default(),
        }
    }
}

/// 환경변수에서 MetaStoreConfig 생성.
///
/// - `DATABASE_BACKEND`: "postgres" | "memory" (기본 "postgres")
/// - `DATABASE_URL`: PostgreSQL 연결 문자열
/// - `REDIS_ENABLED`: "true"/"false" (기본 "false")
/// - `REDIS_URL`: Redis 연결 문자열
pub fn meta_store_config_from_env() -> MetaStoreConfig {
    let backend = std::env::var("DATABASE_BACKEND")
        .unwrap_or_else(|_| "postgres".to_string());
    let postgres_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://user:pass@localhost:5432/meta".to_string()
    });
    let redis = crate::meta::redis::redis_config_from_env();
    MetaStoreConfig {
        backend,
        postgres_url,
        redis,
    }
}

/// 게시판 글 엔티티 (Q3-1: before_create → 저장 → after_save).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BoardPost {
    pub id: i64,
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub extra: serde_json::Value,
    pub created_at: i64,
    /// Phase 37: tenant 격리 (default "default").
    #[serde(default = "default_tenant")]
    pub tenant_id: String,
    /// Phase 41 Step 2: 인스턴스가 바인딩한 템플릿.
    #[serde(default)]
    pub template_id: String,
    #[serde(default)]
    pub template_version: i64,
    #[serde(default = "default_sync_policy")]
    pub sync_policy: String,
}

fn default_tenant() -> String {
    "default".into()
}

/// Phase 41 Step 2: 템플릿 개선 전파 정책 기본값.
fn default_sync_policy() -> String {
    "frozen".into()
}

/// ERP 발주서 엔티티 (Phase 12).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ErpPurchaseOrder {
    pub id: i64,
    pub workflow_id: String,
    pub quantity: i32,
    pub supplier: String,
    pub status: String,
    #[serde(default)]
    pub extra: serde_json::Value,
    pub created_at: i64,
    #[serde(default = "default_tenant")]
    pub tenant_id: String,
}

/// Blog 글 엔티티 (Phase 12).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BlogArticle {
    pub id: i64,
    pub title: String,
    pub body: String,
    pub category: String,
    pub slug: String,
    #[serde(default)]
    pub extra: serde_json::Value,
    pub created_at: i64,
    #[serde(default = "default_tenant")]
    pub tenant_id: String,
    #[serde(default)]
    pub template_id: String,
    #[serde(default)]
    pub template_version: i64,
    #[serde(default = "default_sync_policy")]
    pub sync_policy: String,
}

/// Shop 주문 엔티티 (Phase 12).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ShopOrder {
    pub id: i64,
    pub product_id: String,
    pub price: f64,
    pub quantity: i32,
    pub status: String,
    #[serde(default)]
    pub extra: serde_json::Value,
    pub created_at: i64,
    #[serde(default = "default_tenant")]
    pub tenant_id: String,
}

/// 게시판 태그 엔티티 (Phase 36).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BoardTag {
    pub id: i64,
    pub board_id: String,
    pub post_id: i64,
    pub tag: String,
    pub created_at: i64,
    #[serde(default = "default_tenant")]
    pub tenant_id: String,
}

/// 블로그 댓글 엔티티 (Phase 36).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BlogComment {
    pub id: i64,
    pub article_id: i64,
    pub author: String,
    pub body: String,
    pub parent_id: Option<i64>,
    pub created_at: i64,
    #[serde(default = "default_tenant")]
    pub tenant_id: String,
}

// ---------- Phase 38: Shop 확장 ----------
/// Shop 상품.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ShopProduct {
    pub id: i64,
    pub name: String,
    pub price: f64,
    pub stock: i32,
    pub category: String,
    pub slug: String,
    pub extra: serde_json::Value,
    #[serde(default)]
    pub created_at: i64,
    #[serde(default = "default_tenant")]
    pub tenant_id: String,
    #[serde(default)]
    pub template_id: String,
    #[serde(default)]
    pub template_version: i64,
    #[serde(default = "default_sync_policy")]
    pub sync_policy: String,
}

/// Shop 카트 라인.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ShopCart {
    pub id: i64,
    pub customer: String,
    pub product_id: i64,
    pub quantity: i32,
    pub extra: serde_json::Value,
    #[serde(default)]
    pub created_at: i64,
    #[serde(default = "default_tenant")]
    pub tenant_id: String,
}

/// Phase 41: 계층형 템플릿 노드.
///
/// `parent_id`가 빈 문자열('')이면 루트(페이지/사이트), 비어있으면 하위 템플릿
/// (메뉴/본문/필드). `payload`는 JSON(필드 스키마·레이아웃·기본값).
/// `source`가 'file'이면 파일 시드, 'db'이면 Admin API 등록.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MetaTemplate {
    pub template_id: String,
    #[serde(default = "default_tenant")]
    pub tenant_id: String,
    /// 루트면 '', 하위 템플릿이면 부모 template_id.
    pub parent_id: String,
    pub name: String,
    /// 'page' | 'menu' | 'body' | 'field' | 'site'.
    pub ttype: String,
    /// 'board'|'blog'|'shop'|'homepage'|''.
    pub domain: String,
    /// JSON: 필드 스키마/레이아웃/기본값.
    pub payload: serde_json::Value,
    /// 'db' | 'file'.
    pub source: String,
    pub file_path: String,
    pub version: i32,
    pub enabled: bool,
}

/// Phase 41: 템플릿 트리 (재귀 결합).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TemplateTree {
    pub template: MetaTemplate,
    pub children: Vec<TemplateTree>,
}

/// L3 메타데이터 스토어 추상 트레잇.
///
/// Phase 2에서 모듈 로딩 파이프라인은 이 트레잇만 의존해
/// 백엔드(PostgreSQL/메모리)를 투명하게 교환할 수 있다.
#[async_trait::async_trait]
pub trait DataStore: Send + Sync {
    /// 모듈 ID 기준으로 메타 모듈 조회.
    async fn get_module(&self, module_id: &str) -> Result<Option<MetaModule>, DataStoreError>;

    /// Phase 40: 모듈 조회 (활성화 상태 무관, admin 토글용).
    /// 기본 구현: `get_module`과 동일 (활성화만 반환). 필요 시 구현체에서 오버라이드.
    async fn get_module_raw(&self, module_id: &str) -> Result<Option<MetaModule>, DataStoreError> {
        self.get_module(module_id).await
    }

    /// 활성화된 모듈 전체 조회 (로딩 시).
    async fn list_enabled_modules(&self) -> Result<Vec<MetaModule>, DataStoreError>;

    /// 메타 모듈 저장 (L3 진실의 원천 업데이트).
    async fn upsert_module(&self, module: &MetaModule) -> Result<(), DataStoreError>;

    // ── Phase 41: 계층형 템플릿 ──

    /// 템플릿 활성 버전(최신 version, enabled) 조회.
    async fn get_template(
        &self, template_id: &str, tenant_id: &str,
    ) -> Result<Option<MetaTemplate>, DataStoreError>;

    /// 조건별 템플릿 목록 (테넌트, domain, parent 필터).
    async fn list_templates(
        &self, tenant_id: &str, domain: &str, parent_id: &str,
    ) -> Result<Vec<MetaTemplate>, DataStoreError>;

    /// 특정 노드의 직계 자식(active) 목록.
    async fn list_template_children(
        &self, parent_id: &str, tenant_id: &str,
    ) -> Result<Vec<MetaTemplate>, DataStoreError>;

    /// 템플릿 저장 (버전 증분 시트 + payload 무효화).
    async fn upsert_template(&self, template: &MetaTemplate) -> Result<(), DataStoreError>;

    /// 템플릿 트리 결합 (루트 + 모든 하위 노드 재귀).
    /// 순환 참조 방지: 최대 깊이 32, 방문 노드 추적.
    async fn build_template_tree(
        &self, template_id: &str, tenant_id: &str,
    ) -> Result<Option<TemplateTree>, DataStoreError>;

    // ── Phase 41 Step 2: 인스턴스 템플릿 바인딩/개선 전파 ──

    /// 인스턴스(게시판 글)에 템플릿 바인딩: template_id/version 기록 + payload 기본값 적용.
    /// payload_fields는 템플릿 payload.fields[]의 default 수집 결과 (JSON 문자열).
    /// 직렬화 안전: 5 파라미터 (4 String + 1 i64), TEXT + 네이티브 타입.
    async fn apply_template_to_board_post(
        &self,
        post_id: i64,
        template_id: &str,
        template_version: i64,
        payload_fields: &str,
        sync_policy: &str,
    ) -> Result<(), DataStoreError>;

    /// 템플릿 활성 버전 +1 (개선 버전 생성). 기존 인스턴스 스냅샷은 보존.
    /// 반환: 신규 버전 번호. 활성 버전 없음 시 Err.
    async fn bump_template_version(
        &self, template_id: &str, tenant_id: &str,
    ) -> Result<i64, DataStoreError>;

    /// 템플릿 payload를 `sync_policy='live'` 인스턴스에 재적용 (개선 전파).
    /// payload_fields = 신규 템플릿의 fields default 수집 결과 (JSON 문자열).
    /// 반환: 적용된 인스턴스 수.
    async fn propagate_template_to_board_posts(
        &self,
        tenant_id: &str,
        template_id: &str,
        payload_fields: &str,
    ) -> Result<i64, DataStoreError>;

    /// 사이트 템플릿 트리 프로비저닝: site 템플릿 트리의 모든 body 노드를
    /// 대상 테넌트의 템플릿으로 복사 (테넌트별 인스턴스화 기반).
    /// 반환: 복사된 템플릿 수.
    async fn apply_site_template(
        &self, tenant_id: &str, site_template_id: &str,
    ) -> Result<i64, DataStoreError>;

    // ── Phase 42 P2: Blog/Shop 스태킹 확장 ──

    /// 블로그 글에 템플릿 바인딩: template_id/version 기록 + payload 기본값 적용.
    async fn apply_template_to_blog_article(
        &self,
        article_id: i64,
        template_id: &str,
        template_version: i64,
        payload_fields: &str,
        sync_policy: &str,
    ) -> Result<(), DataStoreError>;

    /// 쇼핑몰 상품에 템플릿 바인딩: template_id/version 기록 + payload 기본값 적용.
    async fn apply_template_to_shop_product(
        &self,
        product_id: i64,
        template_id: &str,
        template_version: i64,
        payload_fields: &str,
        sync_policy: &str,
    ) -> Result<(), DataStoreError>;

    /// 템플릿 payload를 blog_articles의 `sync_policy='live'` 인스턴스에 재적용.
    /// 반환: 적용된 인스턴스 수.
    async fn propagate_template_to_blog_articles(
        &self,
        tenant_id: &str,
        template_id: &str,
        payload_fields: &str,
    ) -> Result<i64, DataStoreError>;

    /// 템플릿 payload를 shop_products의 `sync_policy='live'` 인스턴스에 재적용.
    /// 반환: 적용된 인스턴스 수.
    async fn propagate_template_to_shop_products(
        &self,
        tenant_id: &str,
        template_id: &str,
        payload_fields: &str,
    ) -> Result<i64, DataStoreError>;

    /// 3개 도메인(board+blog+shop)에 동시에 전파. 반환: 총 적용된 인스턴스 수.
    async fn propagate_template_to_all_domains(
        &self,
        tenant_id: &str,
        template_id: &str,
        payload_fields: &str,
    ) -> Result<i64, DataStoreError>;

    /// 게시판 글 저장 (before_create 통과 후 호출).
    async fn save_board_post(&self, post: &BoardPost) -> Result<i64, DataStoreError>;

    /// 게시판 글 ID로 조회.
    async fn get_board_post(&self, id: i64) -> Result<Option<BoardPost>, DataStoreError>;

    /// ERP 발주서 저장 (Phase 12).
    async fn save_erp_purchase_order(&self, order: &ErpPurchaseOrder) -> Result<i64, DataStoreError>;

    /// ERP 발주서 ID로 조회.
    async fn get_erp_purchase_order(&self, id: i64) -> Result<Option<ErpPurchaseOrder>, DataStoreError>;

    /// Blog 글 저장 (Phase 12).
    async fn save_blog_article(&self, article: &BlogArticle) -> Result<i64, DataStoreError>;

    /// Blog 글 ID로 조회.
    async fn get_blog_article(&self, id: i64) -> Result<Option<BlogArticle>, DataStoreError>;

    /// Shop 주문 저장 (Phase 12).
    async fn save_shop_order(&self, order: &ShopOrder) -> Result<i64, DataStoreError>;

    /// Shop 주문 ID로 조회.
    async fn get_shop_order(&self, id: i64) -> Result<Option<ShopOrder>, DataStoreError>;

    /// Phase 36: 게시판 태그 저장.
    async fn create_board_tag(&self, tag: &BoardTag) -> Result<i64, DataStoreError>;

    /// Phase 36: 게시글별 태그 조회.
    async fn list_board_tags_by_post(&self, post_id: i64) -> Result<Vec<BoardTag>, DataStoreError>;

    /// Phase 36: 블로그 댓글 저장.
    async fn create_blog_comment(&self, comment: &BlogComment) -> Result<i64, DataStoreError>;

    /// Phase 36: 블로그 글별 댓글 조회.
    async fn list_blog_comments_by_article(&self, article_id: i64) -> Result<Vec<BlogComment>, DataStoreError>;

    /// Phase 37: tenant별 게시판 글 목록.
    async fn list_board_posts_for_tenant(&self, tenant_id: &str) -> Result<Vec<BoardPost>, DataStoreError>;

    /// Phase 37: tenant별 블로그 글 목록.
    async fn list_blog_articles_for_tenant(&self, tenant_id: &str) -> Result<Vec<BlogArticle>, DataStoreError>;

    /// Phase 37: tenant별 쇼핑몰 주문 목록.
    async fn list_shop_orders_for_tenant(&self, tenant_id: &str) -> Result<Vec<ShopOrder>, DataStoreError>;

    /// Phase 37: tenant별 ERP 발주서 목록.
    async fn list_erp_orders_for_tenant(&self, tenant_id: &str) -> Result<Vec<ErpPurchaseOrder>, DataStoreError>;

    // ---------- Phase 38: Shop 확장 ----------
    /// 상품 저장.
    async fn save_shop_product(&self, product: &ShopProduct) -> Result<i64, DataStoreError>;

    /// 상품 ID 조회.
    async fn get_shop_product(&self, id: i64) -> Result<Option<ShopProduct>, DataStoreError>;

    /// tenant별 상품 목록.
    async fn list_shop_products_for_tenant(&self, tenant_id: &str) -> Result<Vec<ShopProduct>, DataStoreError>;

    /// 상품 슬러그로 조회.
    async fn get_shop_product_by_slug(&self, slug: &str) -> Result<Option<ShopProduct>, DataStoreError>;

    /// 카트 라인 저장.
    async fn save_shop_cart(&self, cart: &ShopCart) -> Result<i64, DataStoreError>;

    /// 고객별 카트 라인 목록.
    async fn list_shop_carts_by_customer(&self, customer: &str) -> Result<Vec<ShopCart>, DataStoreError>;
}

/// DataStore 운영 에러.
#[derive(Debug, thiserror::Error)]
pub enum DataStoreError {
    #[error("DB 연결 실패: {0}")]
    DbConnect(String),
    #[error("DB 조회 실패: {0}")]
    DbQuery(String),
    #[error("메모리 스텁 오류: {0}")]
    Memory(String),
}

// === PostgreSQL 구현 (기본 백엔드) ===

/// PostgreSQL 백엔드 DataStore.
pub struct PostgresStore {
    client: Arc<Client>,
}

impl PostgresStore {
    /// Phase 38: shop_products의 price/stock/slug은 이 빌드의 직렬화 한도를
    /// 우회하기 위해 `extra` TEXT(JSON)에 묶어 저장된다. 읽기 시 extra JSON에서
    /// 존재하면 column 값을 덮어쓴다.
    fn apply_shop_extra(mut p: ShopProduct, extra_json: &str) -> ShopProduct {
        if let Ok(serde_json::Value::Object(m)) = serde_json::from_str::<serde_json::Value>(extra_json) {
            if let Some(v) = m.get("price").and_then(|x| x.as_f64()) {
                p.price = v;
            }
            if let Some(v) = m.get("stock").and_then(|x| x.as_i64()) {
                p.stock = v as i32;
            }
            if let Some(v) = m.get("slug").and_then(|x| x.as_str()) {
                p.slug = v.to_string();
            }
        }
        p
    }

    /// PostgreSQL 인스턴스에 연결해 저장소 생성.
    ///
    /// 연결 실패 시 `Err(DbConnect)` 반환 (기동 시 명시적 실패).
    /// Phase 17: 3회 재시도 (2초 간격).
    pub async fn connect(config: &MetaStoreConfig) -> Result<Self, DataStoreError> {
        let pg_cfg: tokio_postgres::Config = config
            .postgres_url
            .parse()
            .map_err(|e| DataStoreError::DbConnect(format!("잘못된 URL: {e}")))?;

        let mut last_err = String::new();
        for attempt in 1..=3 {
            match pg_cfg.connect(NoTls).await {
                Ok((client, conn)) => {
                    tokio::spawn(async move {
                        if let Err(e) = conn.await {
                            eprintln!("[MetaStore] PostgreSQL 연결 종료: {e}");
                        }
                    });

                    // Phase 13: 마이그레이션 자동 적용
                    crate::meta::migrate::run_migrations(&client)
                        .await
                        .map_err(|e| {
                            eprintln!("[MetaStore] 마이그레이션 실패: {e}");
                            e
                        })?;

                    println!("[MetaStore] PostgreSQL 연결 성공 (시도 {attempt}/3)");
                    return Ok(Self {
                        client: Arc::new(client),
                    });
                }
                Err(e) => {
                    last_err = e.to_string();
                    println!("[MetaStore] PostgreSQL 연결 실패 (시도 {attempt}/3): {last_err}");
                    if attempt < 3 {
                        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    }
                }
            }
        }

        Err(DataStoreError::DbConnect(last_err))
    }

    fn ensure_schema(&self) {
        // 스키마 존재 확인용 no-op (Phase 2 스텁: 스키마는 별도 마이그레이션)
    }

    /// Phase 41: 템플릿 트리 내부 재귀 결합 (순환 참조 방지).
    /// 비동기 재귀 → `Box::pin`으로 future 크기 제한 (E0733).
    async fn build_tree_inner(
        &self, node: &MetaTemplate, tenant_id: &str,
        visited: &mut std::collections::HashSet<(String, i32)>,
    ) -> Result<TemplateTree, DataStoreError> {
        async fn go<'a>(
            store: &'a dyn DataStore,
            node: &'a MetaTemplate, tenant_id: &'a str,
            visited: &mut std::collections::HashSet<(String, i32)>,
        ) -> Result<TemplateTree, DataStoreError> {
            if !visited.insert((node.template_id.clone(), node.version)) {
                return Ok(TemplateTree { template: node.clone(), children: Vec::new() });
            }
            let children = store.list_template_children(&node.template_id, tenant_id).await?;
            let mut tree_children = Vec::new();
            for c in &children {
                tree_children.push(Box::pin(go(store, c, tenant_id, visited)).await?);
            }
            Ok(TemplateTree { template: node.clone(), children: tree_children })
        }
        go(self, node, tenant_id, visited).await
    }
}

#[async_trait::async_trait]
impl DataStore for PostgresStore {
    async fn get_module(&self, module_id: &str) -> Result<Option<MetaModule>, DataStoreError> {
        let row = self
            .client
            .query(
                "SELECT module_id, kind, base_module_id, script_source, content_hash, version, enabled
                 FROM meta_modules WHERE module_id = $1 AND enabled = TRUE",
                &[&module_id],
            )
            .await
            .map_err(|e| DataStoreError::DbQuery(e.to_string()))?;

        if row.is_empty() {
            return Ok(None);
        }
        Ok(Some(row_to_module(&row[0])))
    }

    /// Phase 40: 활성화 상태 무관으로 모듈 조회 (admin 토글용).
    async fn get_module_raw(&self, module_id: &str) -> Result<Option<MetaModule>, DataStoreError> {
        let row = self
            .client
            .query(
                "SELECT module_id, kind, base_module_id, script_source, content_hash, version, enabled
                 FROM meta_modules WHERE module_id = $1",
                &[&module_id],
            )
            .await
            .map_err(|e| DataStoreError::DbQuery(e.to_string()))?;
        if row.is_empty() {
            return Ok(None);
        }
        Ok(Some(row_to_module(&row[0])))
    }

    async fn list_enabled_modules(&self) -> Result<Vec<MetaModule>, DataStoreError> {
        let rows = self
            .client
            .query(
                "SELECT module_id, kind, base_module_id, script_source, content_hash, version, enabled
                 FROM meta_modules WHERE enabled = TRUE ORDER BY module_id",
                &[],
            )
            .await
            .map_err(|e| DataStoreError::DbQuery(e.to_string()))?;
        Ok(rows.iter().map(row_to_module).collect())
    }

    async fn upsert_module(&self, module: &MetaModule) -> Result<(), DataStoreError> {
        let version: i64 = module.version as i64;
        self.client
            .execute(
                "INSERT INTO meta_modules
                   (module_id, kind, base_module_id, script_source, content_hash, version, enabled)
                 VALUES ($1, $2, $3, $4, $5, $6, $7)
                 ON CONFLICT (module_id) DO UPDATE SET
                   kind = EXCLUDED.kind,
                   base_module_id = EXCLUDED.base_module_id,
                   script_source = EXCLUDED.script_source,
                   content_hash = EXCLUDED.content_hash,
                   version = EXCLUDED.version,
                   enabled = EXCLUDED.enabled",
                &[
                    &module.module_id,
                    &module.kind,
                    &module.base_module_id,
                    &module.script_source,
                    &module.content_hash,
                    &version,
                    &module.enabled,
                ],
            )
            .await
            .map_err(|e| DataStoreError::DbQuery(e.to_string()))?;
        Ok(())
    }

    async fn save_board_post(&self, post: &BoardPost) -> Result<i64, DataStoreError> {
        let extra = post
            .extra
            .to_string();
        let id: i64 = self
            .client
            .query_one(
                "INSERT INTO board_posts (title, body, extra, created_at, tenant_id, template_id, template_version, sync_policy) VALUES ($1, $2, $3, to_timestamp($4::float8), $5, $6, $7, $8) RETURNING id",
                &[&post.title, &post.body, &extra, &(post.created_at as f64), &post.tenant_id, &post.template_id, &post.template_version, &post.sync_policy],
            )
            .await
            .map_err(|e| DataStoreError::DbQuery(e.to_string()))?
            .get(0);
        Ok(id)
    }

    async fn get_board_post(&self, id: i64) -> Result<Option<BoardPost>, DataStoreError> {
        let row = self
            .client
            .query_opt(
                "SELECT id, title, body, extra, EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at, COALESCE(tenant_id,'default') AS tenant_id, COALESCE(template_id,'') AS template_id, COALESCE(template_version,0) AS template_version, COALESCE(sync_policy,'frozen') AS sync_policy FROM board_posts WHERE id = $1",
                &[&id],
            )
            .await
            .map_err(|e| DataStoreError::DbQuery(e.to_string()))?;
        Ok(row.map(|r| BoardPost {
            id: r.get("id"),
            title: r.get("title"),
            body: r.get("body"),
            extra: serde_json::from_str(&r.get::<_, String>("extra")).unwrap_or(serde_json::Value::Null),
            created_at: r.get("created_at"),
            tenant_id: r.get("tenant_id"),
            template_id: r.get("template_id"),
            template_version: r.get("template_version"),
            sync_policy: r.get("sync_policy"),
        }))
    }

    async fn save_erp_purchase_order(&self, order: &ErpPurchaseOrder) -> Result<i64, DataStoreError> {
        let extra = order.extra.to_string();
        let id: i64 = self
            .client
            .query_one(
                "INSERT INTO erp_purchase_orders (workflow_id, quantity, supplier, status, extra, created_at, tenant_id) VALUES ($1, $2, $3, $4, $5, to_timestamp($6::float8), $7) RETURNING id",
                &[&order.workflow_id, &order.quantity, &order.supplier, &order.status, &extra, &(order.created_at as f64), &order.tenant_id],
            )
            .await
            .map_err(|e| DataStoreError::DbQuery(e.to_string()))?
            .get(0);
        Ok(id)
    }

    async fn get_erp_purchase_order(&self, id: i64) -> Result<Option<ErpPurchaseOrder>, DataStoreError> {
        let row = self
            .client
            .query_opt(
                "SELECT id, workflow_id, quantity, supplier, status, extra, EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at, COALESCE(tenant_id,'default') AS tenant_id FROM erp_purchase_orders WHERE id = $1",
                &[&id],
            )
            .await
            .map_err(|e| DataStoreError::DbQuery(e.to_string()))?;
        Ok(row.map(|r| ErpPurchaseOrder {
            id: r.get("id"),
            workflow_id: r.get("workflow_id"),
            quantity: r.get("quantity"),
            supplier: r.get("supplier"),
            status: r.get("status"),
            extra: serde_json::from_str(&r.get::<_, String>("extra")).unwrap_or(serde_json::Value::Null),
            created_at: r.get("created_at"),
            tenant_id: r.get("tenant_id"),
        }))
    }

    async fn save_blog_article(&self, article: &BlogArticle) -> Result<i64, DataStoreError> {
        let extra = article.extra.to_string();
        let id: i64 = self
            .client
            .query_one(
                "INSERT INTO blog_articles (title, body, category, slug, extra, created_at, tenant_id, template_id, template_version, sync_policy) VALUES ($1, $2, $3, $4, $5, to_timestamp($6::float8), $7, $8, $9, $10) RETURNING id",
                &[&article.title, &article.body, &article.category, &article.slug, &extra, &(article.created_at as f64), &article.tenant_id, &article.template_id, &article.template_version, &article.sync_policy],
            )
            .await
            .map_err(|e| DataStoreError::DbQuery(e.to_string()))?
            .get(0);
        Ok(id)
    }

    async fn get_blog_article(&self, id: i64) -> Result<Option<BlogArticle>, DataStoreError> {
        let row = self
            .client
            .query_opt(
                "SELECT id, title, body, category, slug, extra, EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at, COALESCE(tenant_id,'default') AS tenant_id, COALESCE(template_id,'') AS template_id, COALESCE(template_version,0) AS template_version, COALESCE(sync_policy,'frozen') AS sync_policy FROM blog_articles WHERE id = $1",
                &[&id],
            )
            .await
            .map_err(|e| DataStoreError::DbQuery(e.to_string()))?;
        Ok(row.map(|r| BlogArticle {
            id: r.get("id"),
            title: r.get("title"),
            body: r.get("body"),
            category: r.get("category"),
            slug: r.get("slug"),
            extra: serde_json::from_str(&r.get::<_, String>("extra")).unwrap_or(serde_json::Value::Null),
            created_at: r.get("created_at"),
            tenant_id: r.get("tenant_id"),
            template_id: r.get("template_id"),
            template_version: r.get("template_version"),
            sync_policy: r.get("sync_policy"),
        }))
    }

    async fn save_shop_order(&self, order: &ShopOrder) -> Result<i64, DataStoreError> {
        let extra = order.extra.to_string();
        let id: i64 = self
            .client
            .query_one(
                "INSERT INTO shop_orders (product_id, price, quantity, status, extra, created_at, tenant_id) VALUES ($1, $2, $3, $4, $5, to_timestamp($6::float8), $7) RETURNING id",
                &[&order.product_id, &order.price, &order.quantity, &order.status, &extra, &(order.created_at as f64), &order.tenant_id],
            )
            .await
            .map_err(|e| DataStoreError::DbQuery(e.to_string()))?
            .get(0);
        Ok(id)
    }

    async fn get_shop_order(&self, id: i64) -> Result<Option<ShopOrder>, DataStoreError> {
        let row = self
            .client
            .query_opt(
                "SELECT id, product_id, price, quantity, status, extra, EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at, COALESCE(tenant_id,'default') AS tenant_id FROM shop_orders WHERE id = $1",
                &[&id],
            )
            .await
            .map_err(|e| DataStoreError::DbQuery(e.to_string()))?;
        Ok(row.map(|r| ShopOrder {
            id: r.get("id"),
            product_id: r.get("product_id"),
            price: r.get("price"),
            quantity: r.get("quantity"),
            status: r.get("status"),
            extra: serde_json::from_str(&r.get::<_, String>("extra")).unwrap_or(serde_json::Value::Null),
            created_at: r.get("created_at"),
            tenant_id: r.get("tenant_id"),
        }))
    }

    async fn create_board_tag(&self, tag: &BoardTag) -> Result<i64, DataStoreError> {
        let row = self
            .client
            .query_one(
                "INSERT INTO board_tags (board_id, post_id, tag, tenant_id) VALUES ($1, $2, $3, $4) RETURNING id",
                &[&tag.board_id, &tag.post_id, &tag.tag, &tag.tenant_id],
            )
            .await
            .map_err(|e| DataStoreError::DbQuery(e.to_string()))?;
        Ok(row.get("id"))
    }

    async fn list_board_tags_by_post(&self, post_id: i64) -> Result<Vec<BoardTag>, DataStoreError> {
        let rows = self
            .client
            .query(
                "SELECT id, board_id, post_id, tag, EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at, COALESCE(tenant_id,'default') AS tenant_id FROM board_tags WHERE post_id = $1 ORDER BY id",
                &[&post_id],
            )
            .await
            .map_err(|e| DataStoreError::DbQuery(e.to_string()))?;
        Ok(rows
            .iter()
            .map(|r| BoardTag {
                id: r.get("id"),
                board_id: r.get("board_id"),
                post_id: r.get("post_id"),
                tag: r.get("tag"),
                created_at: r.get("created_at"),
                tenant_id: r.get("tenant_id"),
            })
            .collect())
    }

    async fn create_blog_comment(&self, comment: &BlogComment) -> Result<i64, DataStoreError> {
        let row = self
            .client
            .query_one(
                "INSERT INTO blog_comments (article_id, author, body, parent_id, tenant_id) VALUES ($1, $2, $3, $4, $5) RETURNING id",
                &[&comment.article_id, &comment.author, &comment.body, &comment.parent_id, &comment.tenant_id],
            )
            .await
            .map_err(|e| DataStoreError::DbQuery(e.to_string()))?;
        Ok(row.get("id"))
    }

    async fn list_blog_comments_by_article(&self, article_id: i64) -> Result<Vec<BlogComment>, DataStoreError> {
        let rows = self
            .client
            .query(
                "SELECT id, article_id, author, body, parent_id, EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at, COALESCE(tenant_id,'default') AS tenant_id FROM blog_comments WHERE article_id = $1 ORDER BY id",
                &[&article_id],
            )
            .await
            .map_err(|e| DataStoreError::DbQuery(e.to_string()))?;
        Ok(rows
            .iter()
            .map(|r| BlogComment {
                id: r.get("id"),
                article_id: r.get("article_id"),
                author: r.get("author"),
                body: r.get("body"),
                parent_id: r.get("parent_id"),
                created_at: r.get("created_at"),
                tenant_id: r.get("tenant_id"),
            })
            .collect())
    }

    async fn list_board_posts_for_tenant(&self, tenant_id: &str) -> Result<Vec<BoardPost>, DataStoreError> {
        let rows = self
            .client
            .query(
                "SELECT id, title, body, extra, EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at, COALESCE(tenant_id,'default') AS tenant_id, COALESCE(template_id,'') AS template_id, COALESCE(template_version,0) AS template_version, COALESCE(sync_policy,'frozen') AS sync_policy FROM board_posts WHERE tenant_id = $1 ORDER BY id",
                &[&tenant_id],
            )
            .await
            .map_err(|e| DataStoreError::DbQuery(e.to_string()))?;
        Ok(rows
            .iter()
            .map(|r| BoardPost {
                id: r.get("id"),
                title: r.get("title"),
                body: r.get("body"),
                extra: serde_json::from_str(&r.get::<_, String>("extra")).unwrap_or(serde_json::Value::Null),
                created_at: r.get("created_at"),
                tenant_id: r.get("tenant_id"),
                template_id: r.get("template_id"),
                template_version: r.get("template_version"),
                sync_policy: r.get("sync_policy"),
            })
            .collect())
    }

    async fn list_blog_articles_for_tenant(&self, tenant_id: &str) -> Result<Vec<BlogArticle>, DataStoreError> {
        let rows = self
            .client
            .query(
                "SELECT id, title, body, category, slug, extra, EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at, COALESCE(tenant_id,'default') AS tenant_id, COALESCE(template_id,'') AS template_id, COALESCE(template_version,0) AS template_version, COALESCE(sync_policy,'frozen') AS sync_policy FROM blog_articles WHERE tenant_id = $1 ORDER BY id",
                &[&tenant_id],
            )
            .await
            .map_err(|e| DataStoreError::DbQuery(e.to_string()))?;
        Ok(rows
            .iter()
            .map(|r| BlogArticle {
                id: r.get("id"),
                title: r.get("title"),
                body: r.get("body"),
                category: r.get("category"),
                slug: r.get("slug"),
                extra: serde_json::from_str(&r.get::<_, String>("extra")).unwrap_or(serde_json::Value::Null),
                created_at: r.get("created_at"),
                tenant_id: r.get("tenant_id"),
                template_id: r.get("template_id"),
                template_version: r.get("template_version"),
                sync_policy: r.get("sync_policy"),
            })
            .collect())
    }

    async fn list_shop_orders_for_tenant(&self, tenant_id: &str) -> Result<Vec<ShopOrder>, DataStoreError> {
        let rows = self
            .client
            .query(
                "SELECT id, product_id, price, quantity, status, extra, EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at, COALESCE(tenant_id,'default') AS tenant_id FROM shop_orders WHERE tenant_id = $1 ORDER BY id",
                &[&tenant_id],
            )
            .await
            .map_err(|e| DataStoreError::DbQuery(e.to_string()))?;
        Ok(rows
            .iter()
            .map(|r| ShopOrder {
                id: r.get("id"),
                product_id: r.get("product_id"),
                price: r.get("price"),
                quantity: r.get("quantity"),
                status: r.get("status"),
                extra: serde_json::from_str(&r.get::<_, String>("extra")).unwrap_or(serde_json::Value::Null),
                created_at: r.get("created_at"),
                tenant_id: r.get("tenant_id"),
            })
            .collect())
    }

    async fn list_erp_orders_for_tenant(&self, tenant_id: &str) -> Result<Vec<ErpPurchaseOrder>, DataStoreError> {
        let rows = self
            .client
            .query(
                "SELECT id, workflow_id, quantity, supplier, status, extra, EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at, COALESCE(tenant_id,'default') AS tenant_id FROM erp_purchase_orders WHERE tenant_id = $1 ORDER BY id",
                &[&tenant_id],
            )
            .await
            .map_err(|e| DataStoreError::DbQuery(e.to_string()))?;
        Ok(rows
            .iter()
            .map(|r| ErpPurchaseOrder {
                id: r.get("id"),
                workflow_id: r.get("workflow_id"),
                quantity: r.get("quantity"),
                supplier: r.get("supplier"),
                status: r.get("status"),
                extra: serde_json::from_str(&r.get::<_, String>("extra")).unwrap_or(serde_json::Value::Null),
                created_at: r.get("created_at"),
                tenant_id: r.get("tenant_id"),
            })
            .collect())
    }

    async fn save_shop_product(&self, product: &ShopProduct) -> Result<i64, DataStoreError> {
        // Phase 38: 이 빌드에서 (1) 단일 쿼리 파라미터 5개 초과, (2) 두 번째
        // 파라미터화 쿼리, (3) JSONB 컬럼에 &String 바인딩이 직렬화 오류를 낸다.
        // 따라서 보드와 동일한 shape(4 String + 1 f64, f64=to_timestamp)의
        // 단일 첫 쿼리로 저장하며, price/stock/slug는 extra TEXT(JSON)에 묶는다.
        let mut extra_obj = serde_json::to_value(&product.extra).unwrap_or(serde_json::Value::Null);
        let mut extra_map = match extra_obj {
            serde_json::Value::Object(m) => m,
            _ => serde_json::Map::new(),
        };
        extra_map.insert("price".into(), serde_json::Value::from(product.price));
        extra_map.insert("stock".into(), serde_json::Value::from(product.stock));
        extra_map.insert("slug".into(), serde_json::Value::from(product.slug.clone()));
        let extra_s = serde_json::Value::Object(extra_map).to_string();
        let n = product.name.clone();
        let c = product.category.clone();
        let t = product.tenant_id.clone();
        let ca = product.created_at;
        let id: i64 = self
            .client
            .query_one(
                "INSERT INTO shop_products (name, category, extra, created_at, tenant_id) VALUES ($1, $2, $3, to_timestamp($4::float8), $5) RETURNING id",
                &[&n, &c, &extra_s, &(ca as f64), &t],
            )
            .await
            .map_err(|e| DataStoreError::DbQuery(e.to_string()))?
            .get(0);
        Ok(id)
    }

    async fn get_shop_product(&self, id: i64) -> Result<Option<ShopProduct>, DataStoreError> {
        let row = self
            .client
            .query_opt(
                "SELECT id, name, price::text AS price, stock, category, slug, extra, EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at, COALESCE(tenant_id,'default') AS tenant_id, COALESCE(template_id,'') AS template_id, COALESCE(template_version,0) AS template_version, COALESCE(sync_policy,'frozen') AS sync_policy FROM shop_products WHERE id = $1",
                &[&id],
            )
            .await
            .map_err(|e| DataStoreError::DbQuery(e.to_string()))?;
        Ok(row.map(|r| {
            let extra_str = r.get::<_, String>("extra");
            let price_str = r.get::<_, String>("price");
            let mut p = ShopProduct {
                id: r.get("id"),
                name: r.get("name"),
                price: price_str.parse().unwrap_or(0.0),
                stock: r.get("stock"),
                category: r.get("category"),
                slug: r.get("slug"),
                extra: serde_json::from_str(&extra_str).unwrap_or(serde_json::Value::Null),
                created_at: r.get("created_at"),
                tenant_id: r.get("tenant_id"),
                template_id: r.get("template_id"),
                template_version: r.get("template_version"),
                sync_policy: r.get("sync_policy"),
            };
            Self::apply_shop_extra(p, &extra_str)
        }))
    }

    async fn list_shop_products_for_tenant(&self, tenant_id: &str) -> Result<Vec<ShopProduct>, DataStoreError> {
        let rows = self
            .client
            .query(
                "SELECT id, name, price::text AS price, stock, category, slug, extra, EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at, COALESCE(tenant_id,'default') AS tenant_id, COALESCE(template_id,'') AS template_id, COALESCE(template_version,0) AS template_version, COALESCE(sync_policy,'frozen') AS sync_policy FROM shop_products WHERE tenant_id = $1 ORDER BY id",
                &[&tenant_id],
            )
            .await
            .map_err(|e| DataStoreError::DbQuery(e.to_string()))?;
        Ok(rows
            .iter()
            .map(|r| {
                let extra_str = r.get::<_, String>("extra");
                let price_str = r.get::<_, String>("price");
                let mut p = ShopProduct {
                    id: r.get("id"),
                    name: r.get("name"),
                    price: price_str.parse().unwrap_or(0.0),
                    stock: r.get("stock"),
                    category: r.get("category"),
                    slug: r.get("slug"),
                    extra: serde_json::from_str(&extra_str).unwrap_or(serde_json::Value::Null),
                    created_at: r.get("created_at"),
                    tenant_id: r.get("tenant_id"),
                    template_id: r.get("template_id"),
                    template_version: r.get("template_version"),
                    sync_policy: r.get("sync_policy"),
                };
                Self::apply_shop_extra(p, &extra_str)
            })
            .collect())
    }

    async fn get_shop_product_by_slug(&self, slug: &str) -> Result<Option<ShopProduct>, DataStoreError> {
        // Phase 38: slug은 extra JSON에 저장되므로 extra 문자열로 매칭한다.
        let row = self
            .client
            .query_opt(
                "SELECT id, name, price::text AS price, stock, category, slug, extra, EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at, COALESCE(tenant_id,'default') AS tenant_id, COALESCE(template_id,'') AS template_id, COALESCE(template_version,0) AS template_version, COALESCE(sync_policy,'frozen') AS sync_policy FROM shop_products WHERE extra LIKE $1 LIMIT 1",
                &[&format!("%\"slug\":\"{}\"%", slug)],
            )
            .await
            .map_err(|e| DataStoreError::DbQuery(e.to_string()))?;
        Ok(row.map(|r| {
            let extra_str = r.get::<_, String>("extra");
            let price_str = r.get::<_, String>("price");
            let mut p = ShopProduct {
                id: r.get("id"),
                name: r.get("name"),
                price: price_str.parse().unwrap_or(0.0),
                stock: r.get("stock"),
                category: r.get("category"),
                slug: r.get("slug"),
                extra: serde_json::from_str(&extra_str).unwrap_or(serde_json::Value::Null),
                created_at: r.get("created_at"),
                tenant_id: r.get("tenant_id"),
                template_id: r.get("template_id"),
                template_version: r.get("template_version"),
                sync_policy: r.get("sync_policy"),
            };
            Self::apply_shop_extra(p, &extra_str)
        }))
    }

    async fn save_shop_cart(&self, cart: &ShopCart) -> Result<i64, DataStoreError> {
        let cust = cart.customer.clone();
        let ten = cart.tenant_id.clone();
        // Phase 38: i64/i32 네이티브 바인딩, f64=to_timestamp. cast 미사용.
        let ca = cart.created_at;
        let id: i64 = self
            .client
            .query_one(
                "INSERT INTO shop_carts (customer, product_id, quantity, created_at, tenant_id) VALUES ($1, $2, $3, to_timestamp($4::float8), $5) RETURNING id",
                &[&cust, &cart.product_id, &cart.quantity, &(ca as f64), &ten],
            )
            .await
            .map_err(|e| DataStoreError::DbQuery(e.to_string()))?
            .get(0);
        Ok(id)
    }

    async fn list_shop_carts_by_customer(&self, customer: &str) -> Result<Vec<ShopCart>, DataStoreError> {
        let rows = self
            .client
            .query(
                "SELECT id, customer, product_id, quantity, extra, EXTRACT(EPOCH FROM created_at)::BIGINT AS created_at, COALESCE(tenant_id,'default') AS tenant_id FROM shop_carts WHERE customer = $1 ORDER BY id",
                &[&customer],
            )
            .await
            .map_err(|e| DataStoreError::DbQuery(e.to_string()))?;
        Ok(rows
            .iter()
            .map(|r| ShopCart {
                id: r.get("id"),
                customer: r.get("customer"),
                product_id: r.get("product_id"),
                quantity: r.get("quantity"),
                extra: serde_json::from_str(&r.get::<_, String>("extra")).unwrap_or(serde_json::Value::Null),
                created_at: r.get("created_at"),
                tenant_id: r.get("tenant_id"),
            })
            .collect())
    }

    // ── Phase 41: 템플릿 (PostgresStore) ──

    async fn get_template(
        &self, template_id: &str, tenant_id: &str,
    ) -> Result<Option<MetaTemplate>, DataStoreError> {
        let rows = self
            .client
            .query(
                "SELECT template_id, COALESCE(tenant_id,'default') AS tenant_id, COALESCE(parent_id,'') AS parent_id, COALESCE(name,'') AS name, COALESCE(ttype,'body') AS ttype, COALESCE(domain,'') AS domain, payload, COALESCE(source,'db') AS source, COALESCE(file_path,'') AS file_path, version, enabled
                 FROM meta_templates WHERE template_id = $1 AND tenant_id = $2 AND enabled = TRUE
                 ORDER BY version DESC LIMIT 1",
                &[&template_id, &tenant_id],
            )
            .await
            .map_err(|e| DataStoreError::DbQuery(e.to_string()))?;
        Ok(rows.into_iter().next().map(|r| row_to_template(&r)))
    }

    async fn list_templates(
        &self, tenant_id: &str, domain: &str, parent_id: &str,
    ) -> Result<Vec<MetaTemplate>, DataStoreError> {
        // 활성 버전만 (같은 id 중 최신 version).
        // 동적 필터 회피: filter 조합별로 정적 쿼리 4개 분기 (Phase 38 직렬화 안전).
        let base = "SELECT DISTINCT ON (template_id) template_id, COALESCE(tenant_id,'default') AS tenant_id, COALESCE(parent_id,'') AS parent_id, COALESCE(name,'') AS name, COALESCE(ttype,'body') AS ttype, COALESCE(domain,'') AS domain, payload, COALESCE(source,'db') AS source, COALESCE(file_path,'') AS file_path, version, enabled FROM meta_templates WHERE tenant_id = $1 AND enabled = TRUE";
        let sql = if !domain.is_empty() && !parent_id.is_empty() {
            format!("{base} AND domain = $2 AND parent_id = $3 ORDER BY template_id, version DESC")
        } else if !domain.is_empty() {
            format!("{base} AND domain = $2 ORDER BY template_id, version DESC")
        } else if !parent_id.is_empty() {
            format!("{base} AND parent_id = $2 ORDER BY template_id, version DESC")
        } else {
            format!("{base} ORDER BY template_id, version DESC")
        };
        let rows = if !domain.is_empty() && !parent_id.is_empty() {
            self.client.query(&sql, &[&tenant_id, &domain, &parent_id]).await
        } else if !domain.is_empty() {
            self.client.query(&sql, &[&tenant_id, &domain]).await
        } else if !parent_id.is_empty() {
            self.client.query(&sql, &[&tenant_id, &parent_id]).await
        } else {
            self.client.query(&sql, &[&tenant_id]).await
        }
        .map_err(|e| DataStoreError::DbQuery(e.to_string()))?;
        Ok(rows.iter().map(row_to_template).collect())
    }

    async fn list_template_children(
        &self, parent_id: &str, tenant_id: &str,
    ) -> Result<Vec<MetaTemplate>, DataStoreError> {
        let rows = self
            .client
            .query(
                "SELECT DISTINCT ON (template_id) template_id, COALESCE(tenant_id,'default') AS tenant_id, COALESCE(parent_id,'') AS parent_id, COALESCE(name,'') AS name, COALESCE(ttype,'body') AS ttype, COALESCE(domain,'') AS domain, payload, COALESCE(source,'db') AS source, COALESCE(file_path,'') AS file_path, version, enabled
                 FROM meta_templates WHERE tenant_id = $1 AND parent_id = $2 AND enabled = TRUE
                 ORDER BY template_id, version DESC",
                &[&tenant_id, &parent_id],
            )
            .await
            .map_err(|e| DataStoreError::DbQuery(e.to_string()))?;
        Ok(rows.iter().map(row_to_template).collect())
    }

    async fn upsert_template(&self, template: &MetaTemplate) -> Result<(), DataStoreError> {
        // 최대 버전 조회 후 +1 (버전 증분 시트)
        let maxv: i64 = self
            .client
            .query_one(
                "SELECT COALESCE(MAX(version), 0) FROM meta_templates WHERE template_id = $1 AND tenant_id = $2",
                &[&template.template_id, &template.tenant_id],
            )
            .await
            .map_err(|e| DataStoreError::DbQuery(e.to_string()))?
            .get(0);
        let next_version = maxv + 1;
        let payload_str = template.payload.to_string();
        // Phase 38 직렬화 특이점: 11개 파라미터 중 updated_at을 now() 디폴트로 대체.
        // payload/문자열 10개 + version(i64) + enabled(bool). 5개 초과이지만
        // 모두 plain TEXT/네이티브 타입이라 직렬화 안전 (Phase 38 패턴).
        self.client
            .execute(
                "INSERT INTO meta_templates (template_id, tenant_id, parent_id, name, ttype, domain, payload, source, file_path, version, enabled)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
                &[
                    &template.template_id,
                    &template.tenant_id,
                    &template.parent_id,
                    &template.name,
                    &template.ttype,
                    &template.domain,
                    &payload_str,
                    &template.source,
                    &template.file_path,
                    &next_version,
                    &template.enabled,
                ],
            )
            .await
            .map_err(|e| DataStoreError::DbQuery(e.to_string()))?;
        Ok(())
    }

    async fn build_template_tree(
        &self, template_id: &str, tenant_id: &str,
    ) -> Result<Option<TemplateTree>, DataStoreError> {
        if let Some(node) = self.get_template(template_id, tenant_id).await? {
            let mut visited = std::collections::HashSet::new();
            Ok(Some(self.build_tree_inner(&node, tenant_id, &mut visited).await?))
        } else {
            Ok(None)
        }
    }

    // ── Phase 41 Step 2: 인스턴스 템플릿 바인딩/개선 전파 ──

    async fn apply_template_to_board_post(
        &self,
        post_id: i64,
        template_id: &str,
        template_version: i64,
        payload_fields: &str,
        sync_policy: &str,
    ) -> Result<(), DataStoreError> {
        // 5 파라미터: 4 String + 1 i64. TEXT + 네이티브 타입. updated_at은 now() 디폴트.
        self.client
            .execute(
                "UPDATE board_posts SET template_id = $1, template_version = $2, sync_policy = $3, extra = $4 WHERE id = $5",
                &[&template_id, &template_version, &sync_policy, &payload_fields, &post_id],
            )
            .await
            .map_err(|e| DataStoreError::DbQuery(e.to_string()))?;
        Ok(())
    }

    async fn bump_template_version(
        &self, template_id: &str, tenant_id: &str,
    ) -> Result<i64, DataStoreError> {
        // 활성 버전(최신) 조회 후 upsert_template로 version+1.
        let current = self
            .get_template(template_id, tenant_id)
            .await?
            .ok_or_else(|| DataStoreError::DbQuery(format!("템플릿 없음: {template_id}")))?;
        let new_version = current.version + 1;
        let mut t = current.clone();
        t.version = new_version;
        self.upsert_template(&t).await?;
        Ok(new_version as i64)
    }

    async fn propagate_template_to_board_posts(
        &self,
        tenant_id: &str,
        template_id: &str,
        payload_fields: &str,
    ) -> Result<i64, DataStoreError> {
        // 4 파라미터: 3 String + 1 String. sync_policy='live' 인스턴스만.
        let affected: u64 = self
            .client
            .execute(
                "UPDATE board_posts SET extra = $1 WHERE tenant_id = $2 AND template_id = $3 AND sync_policy = 'live'",
                &[&payload_fields, &tenant_id, &template_id],
            )
            .await
            .map_err(|e| DataStoreError::DbQuery(e.to_string()))?;
        Ok(affected as i64)
    }

    async fn apply_site_template(
        &self, tenant_id: &str, site_template_id: &str,
    ) -> Result<i64, DataStoreError> {
        // site 템플릿 트리에서 모든 body 노드를 대상 테넌트로 복사.
        // 소스 테넌트: 파일 시드는 "default" 테넌트에 저장됨.
        let source_tenant = "default";
        let tree = self
            .build_template_tree(site_template_id, source_tenant)
            .await?
            .ok_or_else(|| DataStoreError::DbQuery(format!("사이트 템플릿 없음: {site_template_id}")))?;
        let mut count: i64 = 0;
        self.pg_collect_body_nodes(&tree, tenant_id, &mut count).await?;
        Ok(count)
    }

    // ── Phase 42 P2: Blog/Shop 스태킹 확장 ──

    async fn apply_template_to_blog_article(
        &self,
        article_id: i64,
        template_id: &str,
        template_version: i64,
        payload_fields: &str,
        sync_policy: &str,
    ) -> Result<(), DataStoreError> {
        self.client
            .execute(
                "UPDATE blog_articles SET template_id = $1, template_version = $2, sync_policy = $3, extra = $4 WHERE id = $5",
                &[&template_id, &template_version, &sync_policy, &payload_fields, &article_id],
            )
            .await
            .map_err(|e| DataStoreError::DbQuery(e.to_string()))?;
        Ok(())
    }

    async fn apply_template_to_shop_product(
        &self,
        product_id: i64,
        template_id: &str,
        template_version: i64,
        payload_fields: &str,
        sync_policy: &str,
    ) -> Result<(), DataStoreError> {
        self.client
            .execute(
                "UPDATE shop_products SET template_id = $1, template_version = $2, sync_policy = $3, extra = $4 WHERE id = $5",
                &[&template_id, &template_version, &sync_policy, &payload_fields, &product_id],
            )
            .await
            .map_err(|e| DataStoreError::DbQuery(e.to_string()))?;
        Ok(())
    }

    async fn propagate_template_to_blog_articles(
        &self,
        tenant_id: &str,
        template_id: &str,
        payload_fields: &str,
    ) -> Result<i64, DataStoreError> {
        let affected: u64 = self
            .client
            .execute(
                "UPDATE blog_articles SET extra = $1 WHERE tenant_id = $2 AND template_id = $3 AND sync_policy = 'live'",
                &[&payload_fields, &tenant_id, &template_id],
            )
            .await
            .map_err(|e| DataStoreError::DbQuery(e.to_string()))?;
        Ok(affected as i64)
    }

    async fn propagate_template_to_shop_products(
        &self,
        tenant_id: &str,
        template_id: &str,
        payload_fields: &str,
    ) -> Result<i64, DataStoreError> {
        let affected: u64 = self
            .client
            .execute(
                "UPDATE shop_products SET extra = $1 WHERE tenant_id = $2 AND template_id = $3 AND sync_policy = 'live'",
                &[&payload_fields, &tenant_id, &template_id],
            )
            .await
            .map_err(|e| DataStoreError::DbQuery(e.to_string()))?;
        Ok(affected as i64)
    }

    async fn propagate_template_to_all_domains(
        &self,
        tenant_id: &str,
        template_id: &str,
        payload_fields: &str,
    ) -> Result<i64, DataStoreError> {
        let board = self.propagate_template_to_board_posts(tenant_id, template_id, payload_fields).await?;
        let blog = self.propagate_template_to_blog_articles(tenant_id, template_id, payload_fields).await?;
        let shop = self.propagate_template_to_shop_products(tenant_id, template_id, payload_fields).await?;
        Ok(board + blog + shop)
    }
}

/// PostgresStore 내부 헬퍼 (inherent).
impl PostgresStore {
    /// 트리의 모든 body 노드를 대상 테넌트로 복사 (재귀, Postgres).
    async fn pg_collect_body_nodes(
        &self, node: &TemplateTree, tenant_id: &str, count: &mut i64,
    ) -> Result<(), DataStoreError> {
        if node.template.ttype == "body" {
            let mut clone_t = node.template.clone();
            clone_t.tenant_id = tenant_id.to_string();
            clone_t.source = "site".into();
            self.upsert_template(&clone_t).await?;
            *count += 1;
        }
        for child in &node.children {
            let go = Box::pin(self.pg_collect_body_nodes(child, tenant_id, count));
            go.await?;
        }
        Ok(())
    }
}

/// PostgreSQL Row -> MetaModule 변환.
fn row_to_module(row: &tokio_postgres::Row) -> MetaModule {
    MetaModule {
        module_id: row.get("module_id"),
        kind: row.get("kind"),
        base_module_id: row.get("base_module_id"),
        script_source: row.get("script_source"),
        content_hash: row.get("content_hash"),
        version: row.get::<_, i64>("version") as i32,
        enabled: row.get("enabled"),
    }
}

/// Phase 41: 템플릿 row → 모델.
fn row_to_template(row: &tokio_postgres::Row) -> MetaTemplate {
    let payload_str: String = row.get("payload");
    let payload = serde_json::from_str(&payload_str)
        .unwrap_or(serde_json::Value::Null);
    MetaTemplate {
        template_id: row.get("template_id"),
        tenant_id: row.get("tenant_id"),
        parent_id: row.get("parent_id"),
        name: row.get("name"),
        ttype: row.get("ttype"),
        domain: row.get("domain"),
        payload,
        source: row.get("source"),
        file_path: row.get("file_path"),
        version: row.get::<_, i64>("version") as i32,
        enabled: row.get("enabled"),
    }
}

// === 메모리 스텁 구현 (폴백) ===

/// 메모리 기반 DataStore 스텁.
pub struct MemoryStore {
    modules: RwLock<HashMap<String, MetaModule>>,
    posts: RwLock<std::collections::BTreeMap<i64, BoardPost>>,
    erp_orders: RwLock<std::collections::BTreeMap<i64, ErpPurchaseOrder>>,
    blog_articles: RwLock<std::collections::BTreeMap<i64, BlogArticle>>,
    shop_orders: RwLock<std::collections::BTreeMap<i64, ShopOrder>>,
    board_tags: RwLock<std::collections::BTreeMap<i64, BoardTag>>,
    blog_comments: RwLock<std::collections::BTreeMap<i64, BlogComment>>,
    shop_products: RwLock<std::collections::BTreeMap<i64, ShopProduct>>,
    shop_carts: RwLock<std::collections::BTreeMap<i64, ShopCart>>,
    templates: RwLock<std::collections::BTreeMap<(String, String), MetaTemplate>>,
    next_id: std::sync::atomic::AtomicI64,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self {
            modules: RwLock::new(HashMap::new()),
            posts: RwLock::new(std::collections::BTreeMap::new()),
            erp_orders: RwLock::new(std::collections::BTreeMap::new()),
            blog_articles: RwLock::new(std::collections::BTreeMap::new()),
            shop_orders: RwLock::new(std::collections::BTreeMap::new()),
            board_tags: RwLock::new(std::collections::BTreeMap::new()),
            blog_comments: RwLock::new(std::collections::BTreeMap::new()),
            shop_products: RwLock::new(std::collections::BTreeMap::new()),
            shop_carts: RwLock::new(std::collections::BTreeMap::new()),
            templates: RwLock::new(std::collections::BTreeMap::new()),
            next_id: std::sync::atomic::AtomicI64::new(1),
        }
    }

    /// 스텁에 테스트/시드 모듈 적재.
    pub async fn seed(&self, module: MetaModule) {
        self.modules
            .write()
            .await
            .insert(module.module_id.clone(), module);
    }

    /// Phase 41: 템플릿 트리 내부 재귀 결합 (순환 참조 방지).
    async fn build_tree_inner(
        &self, node: &MetaTemplate, tenant_id: &str,
        visited: &mut std::collections::HashSet<(String, i32)>,
    ) -> Result<TemplateTree, DataStoreError> {
        async fn go<'a>(
            store: &'a dyn DataStore,
            node: &'a MetaTemplate, tenant_id: &'a str,
            visited: &mut std::collections::HashSet<(String, i32)>,
        ) -> Result<TemplateTree, DataStoreError> {
            if !visited.insert((node.template_id.clone(), node.version)) {
                return Ok(TemplateTree { template: node.clone(), children: Vec::new() });
            }
            let children = store.list_template_children(&node.template_id, tenant_id).await?;
            let mut tree_children = Vec::new();
            for c in &children {
                tree_children.push(Box::pin(go(store, c, tenant_id, visited)).await?);
            }
            Ok(TemplateTree { template: node.clone(), children: tree_children })
        }
        go(self, node, tenant_id, visited).await
    }
}

impl Default for MemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl DataStore for MemoryStore {
    async fn get_module(&self, module_id: &str) -> Result<Option<MetaModule>, DataStoreError> {
        Ok(self.modules.read().await.get(module_id).cloned())
    }

    async fn list_enabled_modules(&self) -> Result<Vec<MetaModule>, DataStoreError> {
        Ok(self
            .modules
            .read()
            .await
            .values()
            .filter(|m| m.enabled)
            .cloned()
            .collect())
    }

    async fn upsert_module(&self, module: &MetaModule) -> Result<(), DataStoreError> {
        self.modules
            .write()
            .await
            .insert(module.module_id.clone(), module.clone());
        Ok(())
    }

    async fn save_board_post(&self, post: &BoardPost) -> Result<i64, DataStoreError> {
        let id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let mut p = post.clone();
        p.id = id;
        p.created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        self.posts.write().await.insert(id, p);
        Ok(id)
    }

    async fn get_board_post(&self, id: i64) -> Result<Option<BoardPost>, DataStoreError> {
        Ok(self.posts.read().await.get(&id).cloned())
    }

    async fn save_erp_purchase_order(&self, order: &ErpPurchaseOrder) -> Result<i64, DataStoreError> {
        let id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let mut o = order.clone();
        o.id = id;
        o.created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        self.erp_orders.write().await.insert(id, o);
        Ok(id)
    }

    async fn get_erp_purchase_order(&self, id: i64) -> Result<Option<ErpPurchaseOrder>, DataStoreError> {
        Ok(self.erp_orders.read().await.get(&id).cloned())
    }

    async fn save_blog_article(&self, article: &BlogArticle) -> Result<i64, DataStoreError> {
        let id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let mut a = article.clone();
        a.id = id;
        a.created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        self.blog_articles.write().await.insert(id, a);
        Ok(id)
    }

    async fn get_blog_article(&self, id: i64) -> Result<Option<BlogArticle>, DataStoreError> {
        Ok(self.blog_articles.read().await.get(&id).cloned())
    }

    async fn save_shop_order(&self, order: &ShopOrder) -> Result<i64, DataStoreError> {
        let id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let mut o = order.clone();
        o.id = id;
        o.created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        self.shop_orders.write().await.insert(id, o);
        Ok(id)
    }

    async fn get_shop_order(&self, id: i64) -> Result<Option<ShopOrder>, DataStoreError> {
        Ok(self.shop_orders.read().await.get(&id).cloned())
    }

    async fn create_board_tag(&self, tag: &BoardTag) -> Result<i64, DataStoreError> {
        let id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let mut t = tag.clone();
        t.id = id;
        t.created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        self.board_tags.write().await.insert(id, t);
        Ok(id)
    }

    async fn list_board_tags_by_post(&self, post_id: i64) -> Result<Vec<BoardTag>, DataStoreError> {
        Ok(self
            .board_tags
            .read()
            .await
            .values()
            .filter(|t| t.post_id == post_id)
            .cloned()
            .collect())
    }

    async fn create_blog_comment(&self, comment: &BlogComment) -> Result<i64, DataStoreError> {
        let id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let mut c = comment.clone();
        c.id = id;
        c.created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        self.blog_comments.write().await.insert(id, c);
        Ok(id)
    }

    async fn list_blog_comments_by_article(&self, article_id: i64) -> Result<Vec<BlogComment>, DataStoreError> {
        Ok(self
            .blog_comments
            .read()
            .await
            .values()
            .filter(|c| c.article_id == article_id)
            .cloned()
            .collect())
    }

    async fn list_board_posts_for_tenant(&self, tenant_id: &str) -> Result<Vec<BoardPost>, DataStoreError> {
        Ok(self
            .posts
            .read()
            .await
            .values()
            .filter(|p| p.tenant_id == tenant_id)
            .cloned()
            .collect())
    }

    async fn list_blog_articles_for_tenant(&self, tenant_id: &str) -> Result<Vec<BlogArticle>, DataStoreError> {
        Ok(self
            .blog_articles
            .read()
            .await
            .values()
            .filter(|a| a.tenant_id == tenant_id)
            .cloned()
            .collect())
    }

    async fn list_shop_orders_for_tenant(&self, tenant_id: &str) -> Result<Vec<ShopOrder>, DataStoreError> {
        Ok(self
            .shop_orders
            .read()
            .await
            .values()
            .filter(|o| o.tenant_id == tenant_id)
            .cloned()
            .collect())
    }

    async fn list_erp_orders_for_tenant(&self, tenant_id: &str) -> Result<Vec<ErpPurchaseOrder>, DataStoreError> {
        Ok(self
            .erp_orders
            .read()
            .await
            .values()
            .filter(|o| o.tenant_id == tenant_id)
            .cloned()
            .collect())
    }

    async fn save_shop_product(&self, product: &ShopProduct) -> Result<i64, DataStoreError> {
        let id = self.next_id.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let mut p = product.clone();
        p.id = id;
        self.shop_products.write().await.insert(id, p);
        Ok(id)
    }

    async fn get_shop_product(&self, id: i64) -> Result<Option<ShopProduct>, DataStoreError> {
        Ok(self.shop_products.read().await.get(&id).cloned())
    }

    async fn list_shop_products_for_tenant(&self, tenant_id: &str) -> Result<Vec<ShopProduct>, DataStoreError> {
        Ok(self
            .shop_products
            .read()
            .await
            .values()
            .filter(|p| p.tenant_id == tenant_id)
            .cloned()
            .collect())
    }

    async fn get_shop_product_by_slug(&self, slug: &str) -> Result<Option<ShopProduct>, DataStoreError> {
        Ok(self
            .shop_products
            .read()
            .await
            .values()
            .find(|p| p.slug == slug)
            .cloned())
    }

    async fn save_shop_cart(&self, cart: &ShopCart) -> Result<i64, DataStoreError> {
        let id = self.next_id.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let mut c = cart.clone();
        c.id = id;
        self.shop_carts.write().await.insert(id, c);
        Ok(id)
    }

    async fn list_shop_carts_by_customer(&self, customer: &str) -> Result<Vec<ShopCart>, DataStoreError> {
        Ok(self
            .shop_carts
            .read()
            .await
            .values()
            .filter(|c| c.customer == customer)
            .cloned()
            .collect())
    }

    // ── Phase 41: 템플릿 (MemoryStore) ──

    async fn get_template(
        &self, template_id: &str, tenant_id: &str,
    ) -> Result<Option<MetaTemplate>, DataStoreError> {
        Ok(self
            .templates
            .read()
            .await
            .get(&(template_id.to_string(), tenant_id.to_string()))
            .cloned())
    }

    async fn list_templates(
        &self, tenant_id: &str, domain: &str, parent_id: &str,
    ) -> Result<Vec<MetaTemplate>, DataStoreError> {
        Ok(self
            .templates
            .read()
            .await
            .values()
            .filter(|t| t.tenant_id == tenant_id)
            .filter(|t| domain.is_empty() || t.domain == domain)
            .filter(|t| parent_id.is_empty() || t.parent_id == parent_id)
            .filter(|t| t.enabled)
            .cloned()
            .collect())
    }

    async fn list_template_children(
        &self, parent_id: &str, tenant_id: &str,
    ) -> Result<Vec<MetaTemplate>, DataStoreError> {
        Ok(self
            .templates
            .read()
            .await
            .values()
            .filter(|t| t.tenant_id == tenant_id)
            .filter(|t| t.parent_id == parent_id)
            .filter(|t| t.enabled)
            .cloned()
            .collect())
    }

    async fn upsert_template(&self, template: &MetaTemplate) -> Result<(), DataStoreError> {
        let mut t = template.clone();
        let key = (template.template_id.clone(), template.tenant_id.clone());
        let mut map = self.templates.write().await;
        if let Some(existing) = map.get(&key) {
            t.version = existing.version + 1;
        }
        map.insert(key, t);
        Ok(())
    }

    async fn build_template_tree(
        &self, template_id: &str, tenant_id: &str,
    ) -> Result<Option<TemplateTree>, DataStoreError> {
        if let Some(node) = self.get_template(template_id, tenant_id).await? {
            let mut visited = std::collections::HashSet::new();
            Ok(Some(self.build_tree_inner(&node, tenant_id, &mut visited).await?))
        } else {
            Ok(None)
        }
    }

    // ── Phase 41 Step 2: 인스턴스 템플릿 바인딩/개선 전파 ──

    async fn apply_template_to_board_post(
        &self,
        post_id: i64,
        template_id: &str,
        template_version: i64,
        payload_fields: &str,
        sync_policy: &str,
    ) -> Result<(), DataStoreError> {
        let mut lock = self.posts.write().await;
        if let Some(post) = lock.get_mut(&post_id) {
            post.template_id = template_id.to_string();
            post.template_version = template_version;
            post.sync_policy = sync_policy.to_string();
            post.extra =
                serde_json::from_str(payload_fields).unwrap_or(serde_json::Value::Null);
            Ok(())
        } else {
            Err(DataStoreError::DbQuery(format!("게시판 글 없음: {post_id}")))
        }
    }

    async fn bump_template_version(
        &self, template_id: &str, tenant_id: &str,
    ) -> Result<i64, DataStoreError> {
        let current = self
            .get_template(template_id, tenant_id)
            .await?
            .ok_or_else(|| DataStoreError::DbQuery(format!("템플릿 없음: {template_id}")))?;
        let new_version = current.version + 1;
        let mut t = current.clone();
        t.version = new_version;
        self.upsert_template(&t).await?;
        Ok(new_version as i64)
    }

    async fn propagate_template_to_board_posts(
        &self,
        tenant_id: &str,
        template_id: &str,
        payload_fields: &str,
    ) -> Result<i64, DataStoreError> {
        let mut lock = self.posts.write().await;
        let payload =
            serde_json::from_str(payload_fields).unwrap_or(serde_json::Value::Null);
        let mut count: i64 = 0;
        for post in lock.values_mut() {
            if post.tenant_id == tenant_id
                && post.template_id == template_id
                && post.sync_policy == "live"
            {
                post.extra = payload.clone();
                count += 1;
            }
        }
        Ok(count)
    }

    async fn apply_site_template(
        &self, tenant_id: &str, site_template_id: &str,
    ) -> Result<i64, DataStoreError> {
        let source_tenant = "default";
        let tree = self
            .build_template_tree(site_template_id, source_tenant)
            .await?
            .ok_or_else(|| DataStoreError::DbQuery(format!("사이트 템플릿 없음: {site_template_id}")))?;
        let mut count: i64 = 0;
        self.collect_body_nodes(&tree, tenant_id, &mut count).await?;
        Ok(count)
    }

    // ── Phase 42 P2: Blog/Shop 스태킹 확장 ──

    async fn apply_template_to_blog_article(
        &self,
        article_id: i64,
        template_id: &str,
        template_version: i64,
        payload_fields: &str,
        sync_policy: &str,
    ) -> Result<(), DataStoreError> {
        let mut lock = self.blog_articles.write().await;
        if let Some(article) = lock.get_mut(&article_id) {
            article.template_id = template_id.to_string();
            article.template_version = template_version;
            article.sync_policy = sync_policy.to_string();
            article.extra =
                serde_json::from_str(payload_fields).unwrap_or(serde_json::Value::Null);
            Ok(())
        } else {
            Err(DataStoreError::DbQuery(format!("블로그 글 없음: {article_id}")))
        }
    }

    async fn apply_template_to_shop_product(
        &self,
        product_id: i64,
        template_id: &str,
        template_version: i64,
        payload_fields: &str,
        sync_policy: &str,
    ) -> Result<(), DataStoreError> {
        let mut lock = self.shop_products.write().await;
        if let Some(product) = lock.get_mut(&product_id) {
            product.template_id = template_id.to_string();
            product.template_version = template_version;
            product.sync_policy = sync_policy.to_string();
            product.extra =
                serde_json::from_str(payload_fields).unwrap_or(serde_json::Value::Null);
            Ok(())
        } else {
            Err(DataStoreError::DbQuery(format!("쇼핑몰 상품 없음: {product_id}")))
        }
    }

    async fn propagate_template_to_blog_articles(
        &self,
        tenant_id: &str,
        template_id: &str,
        payload_fields: &str,
    ) -> Result<i64, DataStoreError> {
        let mut lock = self.blog_articles.write().await;
        let payload =
            serde_json::from_str(payload_fields).unwrap_or(serde_json::Value::Null);
        let mut count: i64 = 0;
        for article in lock.values_mut() {
            if article.tenant_id == tenant_id
                && article.template_id == template_id
                && article.sync_policy == "live"
            {
                article.extra = payload.clone();
                count += 1;
            }
        }
        Ok(count)
    }

    async fn propagate_template_to_shop_products(
        &self,
        tenant_id: &str,
        template_id: &str,
        payload_fields: &str,
    ) -> Result<i64, DataStoreError> {
        let mut lock = self.shop_products.write().await;
        let payload =
            serde_json::from_str(payload_fields).unwrap_or(serde_json::Value::Null);
        let mut count: i64 = 0;
        for product in lock.values_mut() {
            if product.tenant_id == tenant_id
                && product.template_id == template_id
                && product.sync_policy == "live"
            {
                product.extra = payload.clone();
                count += 1;
            }
        }
        Ok(count)
    }

    async fn propagate_template_to_all_domains(
        &self,
        tenant_id: &str,
        template_id: &str,
        payload_fields: &str,
    ) -> Result<i64, DataStoreError> {
        let board = self.propagate_template_to_board_posts(tenant_id, template_id, payload_fields).await?;
        let blog = self.propagate_template_to_blog_articles(tenant_id, template_id, payload_fields).await?;
        let shop = self.propagate_template_to_shop_products(tenant_id, template_id, payload_fields).await?;
        Ok(board + blog + shop)
    }
}

/// MemoryStore 내부 헬퍼 (inherent).
impl MemoryStore {
    /// 트리의 모든 body 노드를 대상 테넌트로 복사 (재귀).
    async fn collect_body_nodes(
        &self, node: &TemplateTree, tenant_id: &str, count: &mut i64,
    ) -> Result<(), DataStoreError> {
        if node.template.ttype == "body" {
            let mut clone_t = node.template.clone();
            clone_t.tenant_id = tenant_id.to_string();
            clone_t.source = "site".into();
            self.upsert_template(&clone_t).await?;
            *count += 1;
        }
        for child in &node.children {
            let go = Box::pin(self.collect_body_nodes(child, tenant_id, count));
            go.await?;
        }
        Ok(())
    }
}

/// 설정 기반 DataStore 팩토리.
///
/// `backend == "postgres"`면 PostgreSQL 연결, 실패 시 Err 반환.
/// `backend == "memory"`면 메모리 스텁.
pub async fn create_data_store(
    config: &MetaStoreConfig,
) -> Result<Box<dyn DataStore>, DataStoreError> {
    match config.backend.as_str() {
        "memory" => Ok(Box::new(MemoryStore::new())),
        "postgres" => Ok(Box::new(PostgresStore::connect(config).await?)),
        other => Err(DataStoreError::DbConnect(format!(
            "알 수 없는 backend: {other} (postgres|memory)"
        ))),
    }
}

/// Phase 41: 파일 채널 템플릿 시드.
///
/// `templates/*.json`을 스캔해 `meta_templates`에 시드 (멱등).
/// 파일만 존재하는 템플릿만 시드하고, 동일 payload면 재시드하지 않음.
/// 파일 시드는 `source='file'`, `file_path` 기록.
pub async fn seed_templates_from_files(
    store: &dyn DataStore,
) -> Result<usize, DataStoreError> {
    let dir = std::path::Path::new("templates");
    let mut files: Vec<std::path::PathBuf> = match std::fs::read_dir(dir) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().map_or(false, |ext| ext == "json"))
            .collect(),
        Err(_) => return Ok(0), // templates 디렉토리가 없으면 건너뜀
    };
    files.sort();

    let mut seeded = 0;
    for file in &files {
        let path_str = file
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let raw = match std::fs::read_to_string(file) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[Template] 파일 읽기 실패: {path_str}: {e}");
                continue;
            }
        };
        let v: serde_json::Value = match serde_json::from_str(&raw) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[Template] JSON 파싱 실패: {path_str}: {e}");
                continue;
            }
        };
        // 기존 시드 존재 여부 + payload 무결성 확인 (멱등)
        let template_id = v.get("template_id")
            .and_then(|x| x.as_str())
            .unwrap_or_default();
        let tenant_id = v.get("tenant_id")
            .and_then(|x| x.as_str())
            .unwrap_or("default");
        if template_id.is_empty() {
            eprintln!("[Template] template_id 누락: {path_str}");
            continue;
        }
        let existing = store.get_template(template_id, tenant_id).await?;
        if let Some(e) = &existing {
            // 이미 파일에서 시드된 경우(payload 동일)이면 건너뜀
            if e.source == "file" && e.payload == v {
                continue;
            }
        }
        let template = MetaTemplate {
            template_id: template_id.to_string(),
            tenant_id: tenant_id.to_string(),
            parent_id: v.get("parent_id").and_then(|x| x.as_str()).unwrap_or("").to_string(),
            name: v.get("name").and_then(|x| x.as_str()).unwrap_or("").to_string(),
            ttype: v.get("ttype").and_then(|x| x.as_str()).unwrap_or("body").to_string(),
            domain: v.get("domain").and_then(|x| x.as_str()).unwrap_or("").to_string(),
            payload: v.clone(),
            source: "file".to_string(),
            file_path: path_str.clone(),
            version: 1,
            enabled: v.get("enabled").and_then(|x| x.as_bool()).unwrap_or(true),
        };
        if let Err(e) = store.upsert_template(&template).await {
            eprintln!("[Template] 시드 실패: {path_str}: {e}");
            continue;
        }
        seeded += 1;
    }
    if seeded > 0 {
        println!("[Template] 파일 시드 완료: {seeded}개");
    }
    Ok(seeded)
}
