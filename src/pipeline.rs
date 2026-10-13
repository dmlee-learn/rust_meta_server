//! 게시판 글 생성 파이프라인 (Phase 3 Q1-1, Q3-1, Q4-1).
//!
//! 요청 흐름:
//! 1. `POST /api/board` 수신 (JSON: { title, body, extra })
//! 2. L1 캐시에서 `base.board` + `ext.notice` 모듈 스냅샷 로드 (Single Flight)
//! 3. Base→Extension 바인딩 (SandboxedContext)
//! 4. `before_create` 훅 발동 (유효성 검증)
//! 5. 통과 시 `DataStore.save_board_post` (메모리 스텁)
//! 6. `after_save` 훅 발동
//! 7. 응답 { id, ... }
//!
//! 에러 매핑 (Q1-3): CJ1101(구문) → 422, CJ1102(CPU) → 504, CJ1103(메모리) → 503, CJ1104(비즈니스) → 400.

use axum::http::StatusCode;
use axum::Json;
use std::sync::Arc;

use crate::cache::L1Cache;
use crate::core::error::InfraError;
use crate::engine::ModuleLoader;
use crate::meta::store::{BoardPost, DataStore};
use crate::observability::Observability;

/// 게시판 글 생성 요청.
#[derive(serde::Deserialize)]
pub struct CreatePostRequest {
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub extra: serde_json::Value,
}

/// 게시판 글 생성 응답.
#[derive(Debug, serde::Serialize)]
pub struct CreatePostResponse {
    pub code: String,
    pub id: i64,
    pub title: String,
}

/// 게시판 파이프라인 빌더 (L3 DataStore 기반).
///
/// `scripts/base/board.js` + `scripts/extensions/notice.js`을 L3 DataStore에
/// 시드하고, L1 캐시 + 옵저버를 연결해 파이프라인을 구성.
///
/// Phase 4: L3 DataStore(PostgreSQL/메모리 스텁)을 받아 사용.
pub async fn build_board_pipeline_with_store(
    engine: &crate::engine::Engine,
    store: Arc<dyn DataStore>,
    redis_store: Arc<dyn crate::meta::redis::RedisLikeStore>,
    observability: Arc<Observability>,
) -> std::result::Result<BoardPipeline, crate::meta::store::DataStoreError> {
    use crate::engine::ThrottleConfig;
    use crate::meta::store::MetaModule;
    use std::fs;

    let store_box: Arc<dyn DataStore> = store;

    // scripts/ 로 시드 (없으면 기본 샘플)
    let base_src = fs::read_to_string("scripts/base/board.js")
        .unwrap_or_else(|_| {
            "var api = {};\napi.before_create = function(p){ if(!p.title){ throw new Error(JSON.stringify({code:\"SJ1201\",message:\"제목 필수\"})); } return p; };\napi.after_save = function(e){ return e; };\n".to_string()
        });
    let ext_src = fs::read_to_string("scripts/extensions/notice.js")
        .unwrap_or_else(|_| {
            "var api = {};\napi.before_create = function(p){ var v = Base.before_create(p); if(typeof v.pinned === 'undefined'){ v.pinned = false; } return v; };\n".to_string()
        });

    store_box
        .upsert_module(&MetaModule {
            module_id: "base.board".into(),
            kind: "base".into(),
            base_module_id: None,
            script_source: base_src,
            content_hash: "base_h1".into(),
            version: 1,
            enabled: true,
        })
        .await?;
    store_box
        .upsert_module(&MetaModule {
            module_id: "ext.notice".into(),
            kind: "extension".into(),
            base_module_id: Some("base.board".into()),
            script_source: ext_src,
            content_hash: "ext_h1".into(),
            version: 1,
            enabled: true,
        })
        .await?;

    // L1 캐시 + 로더 (Phase 7: L2 Redis 연동)
    let cache = Arc::new(L1Cache::new(store_box.clone()).with_redis(redis_store));
    let loader = ModuleLoader::new(engine.clone(), ThrottleConfig::default());

    Ok(BoardPipeline {
        loader: ModuleLoaderRef::new(loader),
        cache,
        store: store_box,
        observability,
    })
}

/// 파이프라인 컨텍스트: 요청 처리에 필요한 의존성.
///
/// `ModuleLoader`는 Clone 불가 (DataStore Box<dyn>)이므로 참조로 보관.
#[derive(Clone)]
pub struct BoardPipeline {
    pub loader: ModuleLoaderRef,
    pub cache: Arc<L1Cache>,
    pub store: Arc<dyn DataStore>,
    pub observability: Arc<Observability>,
}

/// ModuleLoader 참조 래퍼.
#[derive(Clone)]
pub struct ModuleLoaderRef {
    inner: Arc<ModuleLoader>,
}

impl ModuleLoaderRef {
    pub fn new(loader: ModuleLoader) -> Self {
        Self {
            inner: Arc::new(loader),
        }
    }

    pub fn bind_extension(
        &self,
        base: &crate::meta::store::MetaModule,
        ext: &crate::meta::store::MetaModule,
    ) -> Result<crate::engine::SandboxedContext, crate::engine::loader::LoadError> {
        self.inner.bind_extension(base, ext)
    }

    /// Phase 40: 스크립트 소스 구문 검증 (배포 전 컴파일 체크).
    pub fn compile_validate(&self, source: &str) -> Result<(), crate::engine::loader::LoadError> {
        self.inner.compile_validate_public(source)
    }
}

impl BoardPipeline {
    /// 요청 처리.
    ///
    /// 성공: `Ok((StatusCode, Json))`. 실패: `Err((StatusCode, Json 에러))`.
    pub async fn handle_create(
        &self,
        req: CreatePostRequest,
    ) -> Result<(StatusCode, Json<CreatePostResponse>), (StatusCode, Json<serde_json::Value>)> {
        // Phase 25: Input Validation
        if req.title.trim().is_empty() {
            return Err(self.err(
                StatusCode::UNPROCESSABLE_ENTITY,
                "CJ1201",
                "title은 1자 이상이어야 합니다.",
            ));
        }
        if req.title.len() > 200 {
            return Err(self.err(
                StatusCode::UNPROCESSABLE_ENTITY,
                "CJ1201",
                "title은 200자를 초과할 수 없습니다.",
            ));
        }
        if req.body.len() > 10000 {
            return Err(self.err(
                StatusCode::UNPROCESSABLE_ENTITY,
                "CJ1201",
                "body는 10000자를 초과할 수 없습니다.",
            ));
        }

        // 1. 모듈 로딩 (L1 → L3, Single Flight)
        let base = self
            .cache
            .get_module("base.board")
            .await
            .ok_or_else(|| self.err(StatusCode::NOT_FOUND, "CR0004", "기준 모듈을 찾을 수 없습니다."))?
            .module;
        let ext = self
            .cache
            .get_module("ext.notice")
            .await
            .ok_or_else(|| self.err(StatusCode::NOT_FOUND, "CR0004", "확장 모듈을 찾을 수 없습니다."))?
            .module;

        // 2. Base→Extension 바인딩 + 옵저버 연결
        let sb = self
            .loader
            .bind_extension(&base, &ext)
            .map_err(|e| self.map_load_error(&e))?
            .with_observability(&ext.module_id, ext.version as u64, self.observability.clone());

        // 3. before_create 훅 발동 (옵저버 보고 포함)
        let payload = serde_json::json!({
            "title": req.title,
            "body": req.body,
            "extra": req.extra,
        });
        let payload_str = payload.to_string();
        let hook_script = format!(
            r#"
            (function(){{
                var raw = {};
                var out = Extension.before_create ? Extension.before_create(JSON.parse(raw)) : JSON.parse(raw);
                return JSON.stringify(out);
            }})()
            "#,
            serde_json::json!(payload_str)
        );

        // 옵저버 연결된 eval
        let result_str: String = self
            .with_observer_eval(&sb, &hook_script)
            .await
            .map_err(|e| self.map_infra_error(&e))?;

        let validated: serde_json::Value = serde_json::from_str(&result_str).map_err(|_| {
            self.err(StatusCode::INTERNAL_SERVER_ERROR, "CJ1104", "훅 결과가 JSON이 아닙니다.")
        })?;

        // 4. 저장 (before_create 통과 후)
        let post = BoardPost {
            id: 0,
            title: validated.get("title").and_then(|t| t.as_str()).unwrap_or(&req.title).to_string(),
            body: validated.get("body").and_then(|b| b.as_str()).unwrap_or(&req.body).to_string(),
            extra: validated.get("extra").cloned().unwrap_or(serde_json::Value::Null),
            created_at: 0,
            tenant_id: "default".into(),
            template_id: String::new(),
            template_version: 0,
            sync_policy: String::from("frozen"),
        };
        let id = self
            .store
            .save_board_post(&post)
            .await
            .map_err(|_| self.err(StatusCode::INTERNAL_SERVER_ERROR, "CR0005", "글 저장 실패."))?;

        // 5. after_save 훅 발동 (옵저버 보고 포함)
        let after_script = format!(
            r#"
            (function(){{
                var event = {{ id: {}, title: "{}" }};
                var r = Extension.after_save ? Extension.after_save(event) : event;
                return JSON.stringify(r);
            }})()
            "#,
            id,
            post.title.replace('"', "\\\"")
        );
        let _ = self
            .with_observer_eval(&sb, &after_script)
            .await
            .map_err(|e| self.map_infra_error(&e))?;

        Ok((
            StatusCode::CREATED,
            Json(CreatePostResponse {
                code: "OK".into(),
                id,
                title: post.title,
            }),
        ))
    }

    /// 옵저버 연결된 eval: 모듈 ID/버전을 바인딩하고 eval_string_observed 호출.
    /// Phase 20: spawn_blocking에서 실행 (rquickjs C 라이브러리 시그널 문제 방지).
    async fn with_observer_eval(
        &self,
        sb: &crate::engine::SandboxedContext,
        script: &str,
    ) -> Result<String, InfraError> {
        // SandboxedContext는 Clone을 지원하지 않으므로,
        // eval_string_observed는 &self를 받으므로 직접 호출.
        // spawn_blocking 대신 sync 호출 (rquickjs C 라이브러리 문제 해결을 위해).
        sb.eval_string_observed(script)
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
                self.err(StatusCode::INTERNAL_SERVER_ERROR, "CR0001", "메모리 부족/스레드 할당 실패.")
            }
            InfraError::Cr002 => self.err(StatusCode::INTERNAL_SERVER_ERROR, "CR0002", "엔진 오류."),
            InfraError::Cj102 => self.err(StatusCode::GATEWAY_TIMEOUT, "CJ1102", "CPU Timeout."),
            InfraError::Cj103 => self.err(StatusCode::SERVICE_UNAVAILABLE, "CJ1103", "메모리 초과."),
            InfraError::Cj104 => {
                // CJ1104: 스크립트 throw — JSON 에러 코드가 있으면 그대로, 없으면 일반
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
}

// 사용 안 함 방지: Undefined, axum response
#[allow(dead_code)]
fn _unused() {}
