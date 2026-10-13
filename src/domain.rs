//! DomainPipeline: 도메인별 파이프라인 추상화 (Phase 12).
//!
//! BoardPipeline의 핵심 로직을 일반화한 구조.
//! 도메인별 Base + Extension 모듈을 로드하고, 훅을 발동하며,
//! DataStore에 저장한다.
//!
//! 사용 예:
//! - Board: base.board + ext.notice, before_create/after_save
//! - ERP: base.erp + ext.erp_po, before_process/after_process
//! - Blog: base.blog + ext.blog_tech, before_publish/after_publish
//! - Shop: base.shop + ext.shop_electronics, before_order/after_order

use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::cache::L1Cache;
use crate::core::error::InfraError;
use crate::engine::{Engine, ModuleLoader, SandboxedContext, ThrottleConfig};
use crate::meta::store::DataStore;
use crate::observability::Observability;

/// 도메인별 파이프라인.
#[derive(Clone)]
pub struct DomainPipeline {
    pub domain: String,
    pub base_module_id: String,
    pub ext_module_id: String,
    pub loader: crate::pipeline::ModuleLoaderRef,
    pub cache: Arc<L1Cache>,
    pub store: Arc<dyn DataStore>,
    pub observability: Arc<Observability>,
}

/// 도메인별 요청 (도메인별 필드는 `payload`에 JSON으로 포함).
#[derive(Debug, Deserialize)]
pub struct DomainRequest {
    #[serde(flatten)]
    pub payload: serde_json::Value,
}

/// 도메인별 응답.
#[derive(Debug, Serialize, Deserialize)]
pub struct DomainResponse {
    pub code: String,
    pub id: i64,
    pub domain: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl DomainPipeline {
    /// 파이프라인 빌더.
    ///
    /// Base + Extension 모듈을 L3에 시드하고, L1 캐시 + 옵저버를 연결.
    pub async fn build(
        engine: &Engine,
        store: Arc<dyn DataStore>,
        redis_store: Arc<dyn crate::meta::redis::RedisLikeStore>,
        observability: Arc<Observability>,
        domain: &str,
        base_module_id: &str,
        ext_module_id: &str,
        base_script: &str,
        ext_script: &str,
    ) -> Result<Self, crate::meta::store::DataStoreError> {
        use crate::meta::store::MetaModule;

        let store_box: Arc<dyn DataStore> = store;

        store_box
            .upsert_module(&MetaModule {
                module_id: base_module_id.into(),
                kind: "base".into(),
                base_module_id: None,
                script_source: base_script.into(),
                content_hash: format!("{base_module_id}_h1"),
                version: 1,
                enabled: true,
            })
            .await?;
        store_box
            .upsert_module(&MetaModule {
                module_id: ext_module_id.into(),
                kind: "extension".into(),
                base_module_id: Some(base_module_id.into()),
                script_source: ext_script.into(),
                content_hash: format!("{ext_module_id}_h1"),
                version: 1,
                enabled: true,
            })
            .await?;

        let cache = Arc::new(L1Cache::new(store_box.clone()).with_redis(redis_store));
        let loader = ModuleLoader::new(engine.clone(), ThrottleConfig::default());

        Ok(Self {
            domain: domain.into(),
            base_module_id: base_module_id.into(),
            ext_module_id: ext_module_id.into(),
            loader: crate::pipeline::ModuleLoaderRef::new(loader),
            cache,
            store: store_box,
            observability,
        })
    }

    /// 요청 처리: Base→Extension 바인딩 → 훅 발동 → 저장 → 응답.
    ///
    /// 훅 이름은 도메인별:
    /// - Board: before_create / after_save
    /// - ERP: before_process / after_process
    /// - Blog: before_publish / after_publish
    /// - Shop: before_order / after_order
    pub async fn handle_request(
        &self,
        payload: serde_json::Value,
    ) -> Result<(StatusCode, Json<DomainResponse>), (StatusCode, Json<serde_json::Value>)> {
        // Phase 25: Input Validation per domain
        if let Err(validation_err) = self.validate_input(&payload) {
            return Err(validation_err);
        }

        // 1. 모듈 로딩 (L1 → L2 → L3, Single Flight)
        let base = self
            .cache
            .get_module(&self.base_module_id)
            .await
            .ok_or_else(|| self.err(StatusCode::NOT_FOUND, "CR0004", "기준 모듈을 찾을 수 없습니다."))?
            .module;
        let ext = self
            .cache
            .get_module(&self.ext_module_id)
            .await
            .ok_or_else(|| self.err(StatusCode::NOT_FOUND, "CR0004", "확장 모듈을 찾을 수 없습니다."))?
            .module;

        // 2. Base→Extension 바인딩
        let sb = self
            .loader
            .bind_extension(&base, &ext)
            .map_err(|e| self.map_load_error(&e))?
            .with_observability(&ext.module_id, ext.version as u64, self.observability.clone());

        // 3. before_* 훅 발동
        let before_hook = self.before_hook_name();
        let hook_script = format!(
            r#"
            (function(){{
                var raw = {};
                var out = Extension.{before_hook} ? Extension.{before_hook}(JSON.parse(raw)) : JSON.parse(raw);
                return JSON.stringify(out);
            }})()
            "#,
            serde_json::json!(payload.to_string())
        );

        let result_str: String = sb
            .eval_string_observed(&hook_script)
            .map_err(|e| self.map_infra_error(&e))?;

        let validated: serde_json::Value =
            serde_json::from_str(&result_str).map_err(|_| {
                self.err(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "CJ1104",
                    "훅 결과가 JSON이 아닙니다.",
                )
            })?;

        // 4. 저장 (도메인별 DataStore 메서드)
        let id = self.save_domain(&validated).await?;

        // 5. after_* 훅 발동
        let after_hook = self.after_hook_name();
        let after_script = format!(
            r#"
            (function(){{
                var event = {{ id: {id} }};
                var r = Extension.{after_hook} ? Extension.{after_hook}(event) : event;
                return JSON.stringify(r);
            }})()
            "#
        );
        let _ = sb
            .eval_string_observed(&after_script)
            .map_err(|e| self.map_infra_error(&e))?;

        Ok((
            StatusCode::CREATED,
            Json(DomainResponse {
                code: "OK".into(),
                id,
                domain: self.domain.clone(),
                message: None,
            }),
        ))
    }

    /// before_* 훅 이름 (도메인별).
    fn before_hook_name(&self) -> &'static str {
        match self.domain.as_str() {
            "board" => "before_create",
            "erp" => "before_process",
            "blog" => "before_publish",
            "shop" => "before_order",
            _ => "before_create",
        }
    }

    /// after_* 훅 이름 (도메인별).
    fn after_hook_name(&self) -> &'static str {
        match self.domain.as_str() {
            "board" => "after_save",
            "erp" => "after_process",
            "blog" => "after_publish",
            "shop" => "after_order",
            _ => "after_save",
        }
    }

    /// 도메인별 저장 (DataStore 메서드 호출).
    async fn save_domain(
        &self,
        validated: &serde_json::Value,
    ) -> Result<i64, (StatusCode, Json<serde_json::Value>)> {
        match self.domain.as_str() {
            "board" => {
                let post = crate::meta::store::BoardPost {
                    id: 0,
                    title: validated
                        .get("title")
                        .and_then(|t| t.as_str())
                        .unwrap_or("")
                        .to_string(),
                    body: validated
                        .get("body")
                        .and_then(|b| b.as_str())
                        .unwrap_or("")
                        .to_string(),
                    extra: validated.get("extra").cloned().unwrap_or(serde_json::Value::Null),
                    created_at: 0,
                    tenant_id: "default".into(),
                    template_id: String::new(),
                    template_version: 0,
                    sync_policy: String::from("frozen"),
                };
                self.store
                    .save_board_post(&post)
                    .await
                    .map_err(|_| self.err(StatusCode::INTERNAL_SERVER_ERROR, "CR0005", "글 저장 실패."))
            }
            "erp" => {
                let order = crate::meta::store::ErpPurchaseOrder {
                    id: 0,
                    workflow_id: validated
                        .get("workflow_id")
                        .and_then(|w| w.as_str())
                        .unwrap_or("")
                        .to_string(),
                    quantity: validated.get("quantity").and_then(|q| q.as_i64()).unwrap_or(0) as i32,
                    supplier: validated
                        .get("supplier")
                        .and_then(|s| s.as_str())
                        .unwrap_or("")
                        .to_string(),
                    status: "pending".into(),
                    extra: validated.get("extra").cloned().unwrap_or(serde_json::Value::Null),
                    created_at: 0,
                    tenant_id: "default".into(),
                };
                self.store
                    .save_erp_purchase_order(&order)
                    .await
                    .map_err(|_| self.err(StatusCode::INTERNAL_SERVER_ERROR, "CR0005", "발주서 저장 실패."))
            }
            "blog" => {
                let article = crate::meta::store::BlogArticle {
                    id: 0,
                    title: validated
                        .get("title")
                        .and_then(|t| t.as_str())
                        .unwrap_or("")
                        .to_string(),
                    body: validated
                        .get("body")
                        .and_then(|b| b.as_str())
                        .unwrap_or("")
                        .to_string(),
                    category: validated
                        .get("category")
                        .and_then(|c| c.as_str())
                        .unwrap_or("")
                        .to_string(),
                    slug: String::new(),
                    extra: validated.get("extra").cloned().unwrap_or(serde_json::Value::Null),
                    created_at: 0,
                    tenant_id: "default".into(),
                    template_id: String::new(),
                    template_version: 0,
                    sync_policy: String::from("frozen"),
                };
                self.store
                    .save_blog_article(&article)
                    .await
                    .map_err(|_| self.err(StatusCode::INTERNAL_SERVER_ERROR, "CR0005", "블로그 저장 실패."))
            }
            "shop" => {
                let order = crate::meta::store::ShopOrder {
                    id: 0,
                    product_id: validated
                        .get("product_id")
                        .and_then(|p| p.as_str())
                        .unwrap_or("")
                        .to_string(),
                    price: validated.get("price").and_then(|p| p.as_f64()).unwrap_or(0.0),
                    quantity: validated.get("quantity").and_then(|q| q.as_i64()).unwrap_or(0) as i32,
                    status: "pending".into(),
                    extra: validated.get("extra").cloned().unwrap_or(serde_json::Value::Null),
                    created_at: 0,
                    tenant_id: "default".into(),
                };
                self.store
                    .save_shop_order(&order)
                    .await
                    .map_err(|_| self.err(StatusCode::INTERNAL_SERVER_ERROR, "CR0005", "주문 저장 실패."))
            }
            _ => Err(self.err(
                StatusCode::BAD_REQUEST,
                "CR0004",
                &format!("알 수 없는 도메인: {}", self.domain),
            )),
        }
    }

    /// LoadError → (StatusCode, Json 에러) 매핑.
    fn map_load_error(
        &self,
        e: &crate::engine::loader::LoadError,
    ) -> (StatusCode, Json<serde_json::Value>) {
        match e {
            crate::engine::loader::LoadError::Syntax(msg) => {
                self.err(StatusCode::UNPROCESSABLE_ENTITY, "CJ1101", &format!("구문 오류: {msg}"))
            }
            crate::engine::loader::LoadError::Engine => {
                self.err(StatusCode::INTERNAL_SERVER_ERROR, "CR0002", "엔진 오류.")
            }
            crate::engine::loader::LoadError::CpuTimeout(msg) => {
                self.err(StatusCode::GATEWAY_TIMEOUT, "CJ1102", &format!("CPU Timeout: {msg}"))
            }
            crate::engine::loader::LoadError::Memory(msg) => {
                self.err(StatusCode::SERVICE_UNAVAILABLE, "CJ1103", &format!("메모리 초과: {msg}"))
            }
        }
    }

    /// InfraError → (StatusCode, Json 에러) 매핑.
    fn map_infra_error(&self, e: &InfraError) -> (StatusCode, Json<serde_json::Value>) {
        match e {
            InfraError::Cr001 => {
                self.err(StatusCode::INTERNAL_SERVER_ERROR, "CR0001", "메모리 부족.")
            }
            InfraError::Cr002 => self.err(StatusCode::INTERNAL_SERVER_ERROR, "CR0002", "엔진 오류."),
            InfraError::Cj102 => self.err(StatusCode::GATEWAY_TIMEOUT, "CJ1102", "CPU Timeout."),
            InfraError::Cj103 => self.err(StatusCode::SERVICE_UNAVAILABLE, "CJ1103", "메모리 초과."),
            InfraError::Cj104 => {
                self.err(StatusCode::BAD_REQUEST, "CJ1104", "비즈니스 예외.")
            }
        }
    }

    fn err(&self, status: StatusCode, code: &str, msg: &str) -> (StatusCode, Json<serde_json::Value>) {
        (
            status,
            Json(serde_json::json!({
                "code": code,
                "message": msg
            })),
        )
    }

    /// Phase 25: Input Validation (도메인별).
    fn validate_input(
        &self,
        payload: &serde_json::Value,
    ) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
        let err = |msg: &str| -> (StatusCode, Json<serde_json::Value>) {
            self.err(StatusCode::UNPROCESSABLE_ENTITY, "CJ1201", msg)
        };

        match self.domain.as_str() {
            "erp" => {
                let workflow_id = payload["workflow_id"].as_str().unwrap_or("");
                let supplier = payload["supplier"].as_str().unwrap_or("");
                let quantity = payload["quantity"].as_i64().unwrap_or(0);
                if workflow_id.trim().is_empty() {
                    return Err(err("workflow_id은 1자 이상이어야 합니다."));
                }
                if workflow_id.len() > 100 {
                    return Err(err("workflow_id은 100자를 초과할 수 없습니다."));
                }
                if supplier.trim().is_empty() {
                    return Err(err("supplier은 1자 이상이어야 합니다."));
                }
                if supplier.len() > 200 {
                    return Err(err("supplier은 200자를 초과할 수 없습니다."));
                }
                if quantity < 1 {
                    return Err(err("quantity는 1 이상이어야 합니다."));
                }
                if quantity > 1000000 {
                    return Err(err("quantity는 1000000을 초과할 수 없습니다."));
                }
            }
            "blog" => {
                let title = payload["title"].as_str().unwrap_or("");
                let category = payload["category"].as_str().unwrap_or("");
                if title.trim().is_empty() {
                    return Err(err("title은 1자 이상이어야 합니다."));
                }
                if title.len() > 200 {
                    return Err(err("title은 200자를 초과할 수 없습니다."));
                }
                if category.trim().is_empty() {
                    return Err(err("category은 1자 이상이어야 합니다."));
                }
                if category.len() > 50 {
                    return Err(err("category은 50자를 초과할 수 없습니다."));
                }
            }
            "shop" => {
                let product_id = payload["product_id"].as_str().unwrap_or("");
                let quantity = payload["quantity"].as_i64().unwrap_or(0);
                let price = payload["price"].as_i64().unwrap_or(0);
                if product_id.trim().is_empty() {
                    return Err(err("product_id은 1자 이상이어야 합니다."));
                }
                if product_id.len() > 100 {
                    return Err(err("product_id은 100자를 초과할 수 없습니다."));
                }
                if quantity < 1 {
                    return Err(err("quantity는 1 이상이어야 합니다."));
                }
                if quantity > 1000000 {
                    return Err(err("quantity는 1000000을 초과할 수 없습니다."));
                }
                if price < 0 {
                    return Err(err("price는 음수가 될 수 없습니다."));
                }
            }
            _ => {}
        }

        Ok(())
    }
}
