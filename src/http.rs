//! Axum HTTP 레이어: 엔진 상태와 JS eval 엔드포인트.
//!
//! `build_router`는 통합/기능 테스트에서 실제 HTTP 요청을 보내
//! 엔진 Throttling을 검증할 수 있게 한다.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::SystemTime;

use axum::{Router, routing::get, response::IntoResponse, Json};
use serde_json::Value;

use crate::domain::DomainPipeline;
use crate::engine::Engine;
use crate::pipeline::{BoardPipeline, CreatePostRequest};
use crate::socket::SocketConfig;
use crate::meta::store::{DataStore, ShopCart, ShopProduct};

/// Phase 41: 템플릿 조회 쿼리 파라미터 (?tenant=&domain=&parent=).
#[derive(serde::Deserialize, Default)]
struct TemplateQuery {
    #[serde(default)]
    tenant: Option<String>,
    #[serde(default)]
    domain: String,
    #[serde(default)]
    parent: String,
}

impl TemplateQuery {
    fn tenant(&self) -> String {
        self.tenant.clone().filter(|t| !t.is_empty()).unwrap_or_else(|| "default".to_string())
    }
}

/// 서버 상태를 전역으로 공유.
#[derive(Clone)]
pub struct AppState {
    pub engine: Engine,
    pub board: Option<Arc<BoardPipeline>>,
    /// 도메인별 파이프라인 (Phase 12).
    pub domains: Vec<Arc<DomainPipeline>>,
    /// 소켓 세션 풀 (Phase 5).
    pub socket_pool: std::sync::Arc<crate::socket::session::SessionPool>,
    /// 소켓 훅 핸들러 (Phase 5).
    pub socket_handler: crate::socket::handler::SocketHandler,
    /// DB 백엔드 (Phase 13).
    pub db_backend: String,
    /// 적용된 마이그레이션 (Phase 13).
    pub migrations: Vec<String>,
    /// Redis 활성 여부 (Phase 13).
    pub redis_enabled: bool,
    /// JWT 시크릿 (Phase 14).
    pub jwt_secret: String,
    /// JWT 비활성화 (Phase 14).
    pub jwt_disabled: bool,
    /// Prometheus Metrics (Phase 14).
    pub metrics: std::sync::Arc<crate::metrics::Metrics>,
    /// Rate Limiter (Phase 15).
    pub rate_limiter: std::sync::Arc<crate::rate_limit::RateLimiter>,
    /// Phase 39: 서킷 브레이커 (5xx 오류 추적).
    pub circuit_breaker: std::sync::Arc<tokio::sync::RwLock<crate::rate_limit::CircuitBreaker>>,
    /// 서버 기동 시각 (Phase 22, uptime 계산용).
    pub started_at: SystemTime,
    /// 에러 로그 링 버퍼 (Phase 23, 최대 100개).
    pub error_logs: std::sync::Arc<std::sync::Mutex<VecDeque<ErrorLogEntry>>>,
}

/// 에러 로그 항목 (Phase 23).
#[derive(Debug, Clone)]
pub struct ErrorLogEntry {
    pub code: String,
    pub message: String,
    pub timestamp: u64,
    pub domain: String,
}

impl AppState {
    /// Phase 36: 첫 스토어 접근 (board 우선, 미르면 domains[0]).
    pub fn first_store(&self) -> Option<std::sync::Arc<dyn crate::meta::store::DataStore>> {
        if let Some(p) = &self.board {
            return Some(p.store.clone());
        }
        self.domains.first().map(|d| d.store.clone())
    }

    /// Phase 40: 첫 파이프라인 접근 (board 우선, 미르면 domains[0]).
    /// store/cache/loader를 owned로 반환 (BoardPipeline/DomainPipeline 공통 필드).
    pub fn first_pipeline(
        &self,
    ) -> Option<
        (
            std::sync::Arc<dyn crate::meta::store::DataStore>,
            std::sync::Arc<crate::cache::L1Cache>,
            crate::pipeline::ModuleLoaderRef,
        ),
    > {
        if let Some(p) = &self.board {
            return Some((p.store.clone(), p.cache.clone(), p.loader.clone()));
        }
        if let Some(d) = self.domains.first() {
            return Some((d.store.clone(), d.cache.clone(), d.loader.clone()));
        }
        None
    }

    /// Phase 36: query string에서 정수 파라미터 추출.
    pub fn query_param_i64(req: &axum::http::Request<axum::body::Body>, key: &str) -> i64 {
        let qs = req.uri().query().unwrap_or("");
        for pair in qs.split('&') {
            if let Some((k, v)) = pair.split_once('=') {
                if k == key {
                    return v.parse().unwrap_or(0);
                }
            }
        }
        0
    }

    /// Phase 24: 에러 로그 기록.
    pub fn record_error(&self, code: &str, message: &str, domain: &str) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let mut logs = self.error_logs.lock().unwrap_or_else(|e| e.into_inner());
        logs.push_back(ErrorLogEntry {
            code: code.to_string(),
            message: message.to_string(),
            timestamp: now,
            domain: domain.to_string(),
        });
        if logs.len() > 100 {
            logs.pop_front();
        }
    }
}

/// 현재 UNIX 초 (i64). 엔티티 created_at 기본값에 사용.
fn now_ts() -> i64 {
    SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// 서버 상태 확인 엔드포인트 (Phase 13 확장).
async fn health(state: axum::extract::State<AppState>) -> axum::response::Json<Value> {
    let usage = state.engine.memory_usage();
    let active_sessions = state.socket_pool.len();

    let body = serde_json::json!({
        "status": "ok",
        "engine": {
            "memory_limit": state.engine.memory_limit(),
            "cpu_timeout_ms": state.engine.cpu_timeout_ms(),
            "memory_usage": {
                "malloc_limit": usage.malloc_limit,
                "malloc_size": usage.malloc_size,
            }
        },
        "database": {
            "backend": state.db_backend,
            "migrations_applied": state.migrations,
        },
        "redis": {
            "enabled": state.redis_enabled,
        },
        "pipelines": {
            "board": if state.board.is_some() { "initialized" } else { "not_initialized" },
            "erp": if state.domains.iter().any(|d| d.domain == "erp") { "initialized" } else { "not_initialized" },
            "blog": if state.domains.iter().any(|d| d.domain == "blog") { "initialized" } else { "not_initialized" },
            "shop": if state.domains.iter().any(|d| d.domain == "shop") { "initialized" } else { "not_initialized" },
        },
        "socket": {
            "active_sessions": active_sessions,
        },
        // Phase 39: 서킷 브레이커 상태.
        "circuit_breaker": {
            "state": circuit_state_str(&state).await,
        }
    });
    axum::response::Json(body)
}

/// Phase 39: 서킷 브레이커 상태를 문자열로 변환.
async fn circuit_state_str(state: &AppState) -> &'static str {
    let cb = state.circuit_breaker.read().await;
    match cb.state() {
        crate::rate_limit::CircuitState::Closed => "closed",
        crate::rate_limit::CircuitState::Open => "open",
        crate::rate_limit::CircuitState::HalfOpen => "half_open",
    }
}

/// Deep Health Check (Phase 33): L3 DataStore + Redis + L1 cache + engine 상태 확인.
///
/// `/health`(lightweight)와 달리 실제 DB/Redis 연결을 확인한다.
async fn health_deep(state: axum::extract::State<AppState>) -> axum::response::Json<Value> {
    let usage = state.engine.memory_usage();

    // L3 DataStore 연결 확인 (모듈 목록 조회)
    let db_status = if let Some(pipeline) = state.board.as_ref() {
        let store = &pipeline.store;
        match store.list_enabled_modules().await {
            Ok(modules) => {
                serde_json::json!({ "status": "ok", "modules_loaded": modules.len() })
            }
            Err(e) => serde_json::json!({ "status": "degraded", "error": e.to_string() }),
        }
    } else if let Some(pipeline) = state.domains.first() {
        let store = &pipeline.store;
        match store.list_enabled_modules().await {
            Ok(modules) => {
                serde_json::json!({ "status": "ok", "modules_loaded": modules.len() })
            }
            Err(e) => serde_json::json!({ "status": "degraded", "error": e.to_string() }),
        }
    } else {
        serde_json::json!({ "status": "degraded", "error": "pipeline not initialized" })
    };

    // L1 cache 상태
    let l1 = if let Some(p) = state.board.as_ref() {
        p.cache.len().await
    } else if let Some(p) = state.domains.first() {
        p.cache.len().await
    } else {
        0
    };

    // Phase 34: Redis PING (enabled일 때)
    let redis_status = if state.redis_enabled {
        if let Some(p) = state.board.as_ref() {
            match p.cache.redis_ping().await {
                Ok(()) => serde_json::json!({ "enabled": true, "status": "ok" }),
                Err(e) => serde_json::json!({ "enabled": true, "status": "down", "error": e.to_string() }),
            }
        } else if let Some(p) = state.domains.first() {
            match p.cache.redis_ping().await {
                Ok(()) => serde_json::json!({ "enabled": true, "status": "ok" }),
                Err(e) => serde_json::json!({ "enabled": true, "status": "down", "error": e.to_string() }),
            }
        } else {
            serde_json::json!({ "enabled": true, "status": "unknown" })
        }
    } else {
        serde_json::json!({ "enabled": false, "status": "disabled" })
    };

    let body = serde_json::json!({
        "status": "ok",
        "database": db_status,
        "redis": redis_status,
        "cache_l1_modules": l1,
        "engine": {
            "memory_limit": state.engine.memory_limit(),
            "memory_usage": {
                "malloc_limit": usage.malloc_limit,
                "malloc_size": usage.malloc_size,
            }
        },
        "database_backend": state.db_backend,
    });
    axum::response::Json(body)
}

/// Prometheus Metrics 엔드포인트 (Phase 14).
async fn metrics_handler(state: axum::extract::State<AppState>) -> axum::response::Response {
    use axum::response::{IntoResponse, Response};

    let usage = state.engine.memory_usage();
    state
        .metrics
        .engine_memory
        .store(usage.malloc_size as i64, std::sync::atomic::Ordering::SeqCst);
    state
        .metrics
        .active_sessions
        .store(state.socket_pool.len() as i64, std::sync::atomic::Ordering::SeqCst);

    let body = state.metrics.encode().await;
    Response::builder()
        .header(axum::http::header::CONTENT_TYPE, "text/plain; version=0.0.4")
        .body(axum::body::Body::from(body))
        .unwrap()
}

/// 엔진의 Throttling 설정과 메모리 사용량을 조회.
async fn engine_info(state: axum::extract::State<AppState>) -> axum::response::Json<Value> {
    let usage = state.engine.memory_usage();
    let body = serde_json::json!({
        "code": "OK",
        "memory_limit": state.engine.memory_limit(),
        "cpu_timeout_ms": state.engine.cpu_timeout_ms(),
        "memory_usage": {
            "malloc_limit": usage.malloc_limit,
            "malloc_size": usage.malloc_size,
            "memory_used_size": usage.memory_used_size,
        }
    });
    axum::response::Json(body)
}

/// 테스트 스크립트를 실행해 엔진이 실제로 JS를 실행하는지 확인.
///
/// `{"script": "<js>"}` 수신 -> 실행 결과 또는 에러 코드 반환.
async fn eval_probe(
    state: axum::extract::State<AppState>,
    axum::extract::Json(payload): axum::extract::Json<Value>,
) -> axum::response::Json<Value> {
    let source = payload
        .get("script")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let sb = match state.engine.create_sandbox() {
        Ok(sb) => sb,
        Err(e) => {
            return axum::response::Json(serde_json::json!({
                "code": e.code(),
                "message": e.message(),
            }));
        }
    };

    let result: i64 = match sb.eval_i64(&source) {
        Ok(v) => v,
        Err(e) => {
            return axum::response::Json(serde_json::json!({
                "code": e.code(),
                "message": e.message(),
            }));
        }
    };

    axum::response::Json(serde_json::json!({
        "code": "OK",
        "result": result
    }))
}

/// Phase 22: Admin 대시보드 페이지 서빙.
/// Phase 42: 토큰 인증 추가 — `?token=` 쿼리 또는 `X-Admin-Token` 헤더.
async fn admin_dashboard(
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> axum::response::Response {
    use crate::admin::dashboard_response;
    use axum::response::{IntoResponse, Response};

    // ADMIN_TOKEN 미설정 → 대시보드 직접 반환 (개발용).
    let expected = match std::env::var("ADMIN_TOKEN") {
        Ok(t) if !t.is_empty() => Some(t),
        _ => None,
    };

    match expected {
        None => dashboard_response(),
        Some(expected_token) => {
            // 1. 쿼리 파라미터 `?token=...` 확인.
            let query_token = q.get("token").map(|s| s.as_str()).unwrap_or("");
            // 2. 헤더 `X-Admin-Token` 확인 (브라우저가 헤더를 보낼 수 없으므로 쿼리 우선).
            if query_token == expected_token {
                dashboard_response()
            } else {
                login_page_response()
            }
        }
    }
}

/// 로그인 페이지 HTML.
fn login_page_response() -> axum::response::Response {
    use axum::response::{IntoResponse, Response};
    const HTML: &str = r#"<!DOCTYPE html>
<html lang="ko">
<head>
<meta charset="utf-8"/>
<title>MetaServer — Admin Login</title>
<style>
:root{--bg:#0d1117;--card:#161b22;--border:#30363d;--text:#c9d1d9;--accent:#58a6ff;--err:#f85149;--ok:#3fb950}
*{box-sizing:border-box;margin:0;padding:0}
body{background:var(--bg);color:var(--text);font-family:'Segoe UI',system-ui,sans-serif;display:flex;align-items:center;justify-content:center;min-height:100vh}
.card{background:var(--card);border:1px solid var(--border);border-radius:12px;padding:40px;width:380px}
.card h1{font-size:20px;margin-bottom:8px}
.card p{font-size:13px;color:#8b949e;margin-bottom:24px}
label{display:block;font-size:13px;margin-bottom:6px;color:#8b949e}
input{width:100%;padding:10px 14px;background:var(--bg);border:1px solid var(--border);border-radius:8px;color:var(--text);font-size:14px;outline:none}
input:focus{border-color:var(--accent)}
button{width:100%;margin-top:16px;padding:12px;background:var(--accent);color:#0d1117;border:none;border-radius:8px;font-size:14px;font-weight:600;cursor:pointer}
button:hover{opacity:.9}
.err{color:var(--err);font-size:12px;margin-top:8px;display:none}
.hint{font-size:11px;color:#8b949e;margin-top:16px;line-height:1.6}
</style>
</head>
<body>
<div class="card">
<h1>⚙️ MetaServer Admin</h1>
<p>관리자 인증이 필요합니다.</p>
<form id="loginForm">
<label for="token">Admin Token</label>
<input type="password" id="token" placeholder="Admin Token 입력" autocomplete="off" required/>
<div class="err" id="err">토큰이 올바르지 않습니다.</div>
<button type="submit">로그인</button>
</form>
<div class="hint">도움이 필요하면 서버 운영자에게 문의하세요.</div>
</div>
<script>
document.getElementById('loginForm').addEventListener('submit',function(e){
e.preventDefault();
var t=document.getElementById('token').value.trim();
if(!t){document.getElementById('err').style.display='block';return;}
window.location.href='/admin?token='+encodeURIComponent(t);
});
</script>
</body>
</html>"#;
    Response::builder()
        .header(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(axum::body::Body::from(HTML))
        .unwrap()
}

/// Phase 22: 종합 서버 상태 JSON.
async fn admin_status(state: axum::extract::State<AppState>) -> axum::response::Json<Value> {
    let usage = state.engine.memory_usage();
    let uptime = crate::admin::uptime_seconds(&state.started_at);
    // Phase 26: L1 cache warmed module count
    let l1_warm = if let Some(b) = &state.board {
        b.cache.len().await
    } else {
        0
    };
    let body = serde_json::json!({
        "status": "ok",
        "uptime_seconds": uptime,
        "timestamp": crate::admin::now_iso(),
        "engine": {
            "memory_limit_mb": state.engine.memory_limit() / 1024 / 1024,
            "cpu_timeout_ms": state.engine.cpu_timeout_ms(),
            "memory_usage": {
                "mb": usage.malloc_size as f64 / 1024.0 / 1024.0,
                "limit_mb": usage.malloc_limit as f64 / 1024.0 / 1024.0
            }
        },
        "database": {
            "backend": state.db_backend,
            "migrations_applied": state.migrations,
            "l1_warm_modules": l1_warm
        },
        "redis": {
            "enabled": state.redis_enabled
        },
        "pipelines": {
            "board": if state.board.is_some() { "initialized" } else { "not_initialized" },
            "erp": if state.domains.iter().any(|d| d.domain == "erp") { "initialized" } else { "not_initialized" },
            "blog": if state.domains.iter().any(|d| d.domain == "blog") { "initialized" } else { "not_initialized" },
            "shop": if state.domains.iter().any(|d| d.domain == "shop") { "initialized" } else { "not_initialized" }
        },
        "socket": {
            "active_sessions": state.socket_pool.len()
        },
        "auth": {
            "jwt_enabled": !state.jwt_disabled,
            "rate_limit_per_min": 100
        },
        "version": "1.0.0"
    });
    axum::response::Json(body)
}

/// Phase 22: 메트릭스 JSON 스냅샷.
async fn admin_metrics(state: axum::extract::State<AppState>) -> axum::response::Json<Value> {
    let usage = state.engine.memory_usage();
    let m = &state.metrics;
    use std::sync::atomic::Ordering;

    let board = m.http_requests.board.load(Ordering::SeqCst);
    let erp = m.http_requests.erp.load(Ordering::SeqCst);
    let blog = m.http_requests.blog.load(Ordering::SeqCst);
    let shop = m.http_requests.shop.load(Ordering::SeqCst);

    let cache_hits = m.cache_hits.l1.load(Ordering::SeqCst)
        + m.cache_hits.l2.load(Ordering::SeqCst)
        + m.cache_hits.l3.load(Ordering::SeqCst);
    let cache_misses = m.cache_misses.l1.load(Ordering::SeqCst)
        + m.cache_misses.l2.load(Ordering::SeqCst)
        + m.cache_misses.l3.load(Ordering::SeqCst);
    let total_cache = cache_hits + cache_misses;
    let hit_rate = if total_cache > 0 {
        (cache_hits as f64 / total_cache as f64) * 100.0
    } else {
        0.0
    };

    let hook_exec = m.hook_executions.load(Ordering::SeqCst);
    let hook_err = m.hook_errors.load(Ordering::SeqCst);

    let body = serde_json::json!({
        "http_requests": {
            "board": board,
            "erp": erp,
            "blog": blog,
            "shop": shop
        },
        "cache": {
            "hits": cache_hits,
            "misses": cache_misses,
            "hit_rate_percent": hit_rate
        },
        "hooks": {
            "executions": hook_exec,
            "errors": hook_err
        },
        "engine_memory_mb": usage.malloc_size as f64 / 1024.0 / 1024.0,
        "active_sessions": state.socket_pool.len()
    });
    axum::response::Json(body)
}

/// Phase 23: 에러 로그 조회 (마지막 50개).
async fn admin_logs(state: axum::extract::State<AppState>) -> axum::response::Json<Value> {
    let logs = state.error_logs.lock().unwrap_or_else(|e| e.into_inner());
    let last_50: Vec<&ErrorLogEntry> = logs.iter().rev().take(50).collect();
    let body = serde_json::json!({
        "total": logs.len(),
        "logs": last_50.iter().map(|l| serde_json::json!({
            "code": l.code,
            "message": l.message,
            "timestamp": l.timestamp,
            "domain": l.domain
        })).collect::<Vec<_>>()
    });
    axum::response::Json(body)
}

/// Phase 23: 캐시 flush (모든 L1 캐시).
async fn admin_cache_flush(
    state: axum::extract::State<AppState>,
) -> axum::response::Json<Value> {
    let mut flushed = 0u32;

    if let Some(ref board) = state.board {
        let entries = board.cache.len().await;
        board.cache.clear().await;
        flushed += 1;
        let _ = entries; // len은 정보용
    }

    for domain in &state.domains {
        let _ = domain.cache.len().await;
        domain.cache.clear().await;
        flushed += 1;
    }

    axum::response::Json(serde_json::json!({
        "code": "OK",
        "flushed": flushed,
        "message": format!("{flushed}개 캐시 플러시 완료")
    }))
}

// =========================================================
// Phase 40: Admin Extension (모듈) Management
// =========================================================

/// Phase 40: 활성화된 모듈 목록 + L1 캐시 상태.
async fn admin_modules_list(state: axum::extract::State<AppState>) -> axum::response::Json<Value> {
    use crate::meta::store::DataStore;
    let (store, cache, _loader) = match state.first_pipeline() {
        Some(p) => p,
        None => {
            return axum::response::Json(serde_json::json!({
                "code": "CR0005",
                "message": "파이프라인이 초기화되지 않았습니다."
            }))
        }
    };
    match store.list_enabled_modules().await {
        Ok(modules) => {
            let mut items: Vec<Value> = Vec::with_capacity(modules.len());
            for m in &modules {
                let cached = cache.get_module(&m.module_id).await.is_some();
                items.push(serde_json::json!({
                    "module_id": m.module_id,
                    "kind": m.kind,
                    "base_module_id": m.base_module_id,
                    "version": m.version,
                    "enabled": m.enabled,
                    "content_hash": m.content_hash,
                    "cached_in_l1": cached,
                    "script_length": m.script_source.chars().count(),
                }));
            }
            axum::response::Json(serde_json::json!({
                "code": "OK",
                "count": items.len(),
                "modules": items
            }))
        }
        Err(e) => {
            state.record_error("CR0005", &e.to_string(), "admin_module");
            axum::response::Json(serde_json::json!({
                "code": "CR0005",
                "message": format!("모듈 목록 조회 실패: {e}")
            }))
        }
    }
}

/// Phase 40: 모듈 상세 조회.
async fn admin_module_get(
    state: axum::extract::State<AppState>,
    axum::extract::Path(module_id): axum::extract::Path<String>,
) -> axum::response::Json<Value> {
    use crate::meta::store::DataStore;
    let (store, cache, _loader) = match state.first_pipeline() {
        Some(p) => p,
        None => {
            return axum::response::Json(serde_json::json!({
                "code": "CR0005",
                "message": "파이프라인이 초기화되지 않았습니다."
            }))
        }
    };
    match store.get_module(&module_id).await {
        Ok(Some(m)) => {
            let cached = cache.get_module(&module_id).await.is_some();
            axum::response::Json(serde_json::json!({
                "code": "OK",
                "module": {
                    "module_id": m.module_id,
                    "kind": m.kind,
                    "base_module_id": m.base_module_id,
                    "version": m.version,
                    "enabled": m.enabled,
                    "content_hash": m.content_hash,
                    "cached_in_l1": cached,
                    "script_source": m.script_source,
                }
            }))
        }
        Ok(None) => axum::response::Json(serde_json::json!({
            "code": "CR4005",
            "message": format!("모듈 '{module_id}'을(를) 찾을 수 없습니다.")
        })),
        Err(e) => {
            state.record_error("CR0005", &e.to_string(), "admin_module");
            axum::response::Json(serde_json::json!({
                "code": "CR0005",
                "message": format!("모듈 조회 실패: {e}")
            }))
        }
    }
}

/// Phase 41: 템플릿 트리 결합 조회 (루트 + 모든 하위 노드).
async fn template_tree(
    state: axum::extract::State<AppState>,
    axum::extract::Path(template_id): axum::extract::Path<String>,
    query: axum::extract::Query<TemplateQuery>,
) -> axum::response::Json<Value> {
    use crate::meta::store::DataStore;
    let (store, cache, _loader) = match state.first_pipeline() {
        Some(p) => p,
        None => {
            return axum::response::Json(serde_json::json!({
                "code": "CR0005",
                "message": "파이프라인이 초기화되지 않았습니다."
            }))
        }
    };
    let tenant = query.0.tenant();

    // Phase 42 P5: Redis 캐시 경로.
    let cache_key_suffix = format!("tree:{}", template_id);
    if let Some(cached) = cache.template_cache_get(&tenant, &cache_key_suffix).await {
        if let Ok(val) = serde_json::from_str::<Value>(&cached) {
            return axum::response::Json(val);
        }
    }

    match store.build_template_tree(&template_id, &tenant).await {
        Ok(Some(tree)) => {
            let resp = serde_json::json!({
                "code": "OK",
                "template": tree
            });
            // Phase 42 P5: Redis 캐시에 저장.
            if let Ok(json_str) = serde_json::to_string(&resp) {
                cache.template_cache_set(&tenant, &cache_key_suffix, &json_str).await;
            }
            axum::response::Json(resp)
        },
        Ok(None) => axum::response::Json(serde_json::json!({
            "code": "CR4006",
            "message": format!("템플릿을 찾을 수 없습니다: {template_id}")
        })),
        Err(e) => {
            state.record_error("CR4006", &e.to_string(), "template");
            axum::response::Json(serde_json::json!({
                "code": "CR0005",
                "message": format!("템플릿 트리 조회 실패: {e}")
            }))
        }
    }
}

/// Phase 41: 템플릿 목록 (도메인/부모 필터).
async fn template_list(
    state: axum::extract::State<AppState>,
    query: axum::extract::Query<TemplateQuery>,
) -> axum::response::Json<Value> {
    use crate::meta::store::DataStore;
    let (store, cache, _loader) = match state.first_pipeline() {
        Some(p) => p,
        None => {
            return axum::response::Json(serde_json::json!({
                "code": "CR0005",
                "message": "파이프라인이 초기화되지 않았습니다."
            }))
        }
    };
    let tenant = query.0.tenant();
    let domain = query.0.domain.clone();
    let parent = query.0.parent.clone();

    // Phase 42 P5: Redis 캐시 경로 (REDIS_ENABLED=true 시).
    let cache_key_suffix = if parent.is_empty() {
        format!("list:{}:{}", domain, tenant)
    } else {
        format!("list:{}:{}:{}", domain, tenant, parent)
    };
    if let Some(cached) = cache.template_cache_get(&tenant, &cache_key_suffix).await {
        if let Ok(val) = serde_json::from_str::<Value>(&cached) {
            return axum::response::Json(val);
        }
    }

    match store.list_templates(&tenant, &domain, &parent).await {
        Ok(items) => {
            let out: Vec<Value> = items.iter().map(|t| {
                serde_json::json!({
                    "template_id": t.template_id,
                    "parent_id": t.parent_id,
                    "name": t.name,
                    "ttype": t.ttype,
                    "domain": t.domain,
                    "source": t.source,
                    "version": t.version,
                    "enabled": t.enabled,
                    "payload": t.payload,
                })
            }).collect();
            let resp = serde_json::json!({
                "code": "OK",
                "count": out.len(),
                "templates": out
            });
            // Phase 42 P5: Redis 캐시에 저장.
            if let Ok(json_str) = serde_json::to_string(&resp) {
                cache.template_cache_set(&tenant, &cache_key_suffix, &json_str).await;
            }
            axum::response::Json(resp)
        }
        Err(e) => {
            state.record_error("CR0005", &e.to_string(), "template");
            axum::response::Json(serde_json::json!({
                "code": "CR0005",
                "message": format!("템플릿 목록 조회 실패: {e}")
            }))
        }
    }
}

/// Phase 41: 템플릿 단일 노드 상세.
async fn template_get(
    state: axum::extract::State<AppState>,
    axum::extract::Path(template_id): axum::extract::Path<String>,
    query: axum::extract::Query<TemplateQuery>,
) -> axum::response::Json<Value> {
    use crate::meta::store::DataStore;
    let (store, cache, _loader) = match state.first_pipeline() {
        Some(p) => p,
        None => {
            return axum::response::Json(serde_json::json!({
                "code": "CR0005",
                "message": "파이프라인이 초기화되지 않았습니다."
            }))
        }
    };
    let tenant = query.0.tenant();

    // Phase 42 P5: Redis 캐시 경로.
    let cache_key_suffix = format!("detail:{}", template_id);
    if let Some(cached) = cache.template_cache_get(&tenant, &cache_key_suffix).await {
        if let Ok(val) = serde_json::from_str::<Value>(&cached) {
            return axum::response::Json(val);
        }
    }

    match store.get_template(&template_id, &tenant).await {
        Ok(Some(t)) => {
            let resp = serde_json::json!({
                "code": "OK",
                "template": {
                    "template_id": t.template_id,
                    "tenant_id": t.tenant_id,
                    "parent_id": t.parent_id,
                    "name": t.name,
                    "ttype": t.ttype,
                    "domain": t.domain,
                    "source": t.source,
                    "file_path": t.file_path,
                    "version": t.version,
                    "enabled": t.enabled,
                    "payload": t.payload,
                }
            });
            // Phase 42 P5: Redis 캐시에 저장.
            if let Ok(json_str) = serde_json::to_string(&resp) {
                cache.template_cache_set(&tenant, &cache_key_suffix, &json_str).await;
            }
            axum::response::Json(resp)
        },
        Ok(None) => axum::response::Json(serde_json::json!({
            "code": "CR4006",
            "message": format!("템플릿을 찾을 수 없습니다: {template_id}")
        })),
        Err(e) => {
            state.record_error("CR0005", &e.to_string(), "template");
            axum::response::Json(serde_json::json!({
                "code": "CR0005",
                "message": format!("템플릿 조회 실패: {e}")
            }))
        }
    }
}

/// Phase 41 Step 2: 템플릿 payload.fields[].default 수집.
/// fields 배열의 각 {key, default}를 key→default 객체로 변환 (없으면 공백).
fn collect_template_fields(payload: &Value) -> Value {
    let mut fields = serde_json::Map::new();
    // fields는 payload.fields 또는 payload.payload.fields 위치에 있을 수 있음.
    let fields_arr = payload.get("fields").and_then(|f| f.as_array())
        .or_else(|| payload.get("payload").and_then(|p| p.get("fields")).and_then(|f| f.as_array()));
    if let Some(arr) = fields_arr {
        for f in arr {
            if let (Some(key), Some(default)) = (
                f.get("key").and_then(|k| k.as_str()),
                f.get("default"),
            ) {
                fields.insert(key.to_string(), default.clone());
            } else if let Some(key) = f.get("key").and_then(|k| k.as_str()) {
                fields.insert(key.to_string(), Value::String(String::new()));
            }
        }
    }
    Value::Object(fields)
}

/// Phase 41 Step 2: 게시판 템플릿 기반 인스턴스 생성.
///
/// 템플릿 payload.fields[].default를 기본값으로 수집해 게시글 생성 +
/// template_id/version/sync_policy 바인딩 기록.
/// 요청 바디의 title/body/extra가 있으면 템플릿 기본값을 덮어씀.
async fn board_instance_create(
    state: axum::extract::State<AppState>,
    query: axum::extract::Query<TemplateInstanceQuery>,
    axum::extract::Json(req): axum::extract::Json<Value>,
) -> axum::response::Json<Value> {
    use crate::meta::store::DataStore;
    let template_id = match query.0.template_id.clone() {
        Some(t) if !t.is_empty() => t,
        _ => {
            return axum::response::Json(serde_json::json!({
                "code": "CJ1202",
                "message": "template_id 쿼리 파라미터가 필요합니다."
            }))
        }
    };
    let sync_policy = if query.0.sync_policy.is_empty() { "frozen" } else { &query.0.sync_policy };

    let (store, _cache, _loader) = match state.first_pipeline() {
        Some(p) => p,
        None => {
            return axum::response::Json(serde_json::json!({
                "code": "CR0005",
                "message": "파이프라인이 초기화되지 않았습니다."
            }))
        }
    };
    let tenant = query.0.tenant();

    // 1. 템플릿 활성 버전 조회.
    let template = match store.get_template(&template_id, &tenant).await {
        Ok(Some(t)) => t,
        Ok(None) => {
            return axum::response::Json(serde_json::json!({
                "code": "CR4006",
                "message": format!("템플릿을 찾을 수 없습니다: {template_id}")
            }))
        }
        Err(e) => {
            state.record_error("CR4006", &e.to_string(), "template");
            return axum::response::Json(serde_json::json!({
                "code": "CR0005",
                "message": format!("템플릿 조회 실패: {e}")
            }))
        }
    };

    // 2. 템플릿 fields default 수집.
    let defaults = collect_template_fields(&template.payload);

    // 3. 요청 바디와 병합 (요청값 우선).
    let merged = |
        key: &str,
    | -> String {
        req.get(key)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| defaults.get(key).and_then(|v| v.as_str()).map(|s| s.to_string()))
            .unwrap_or_default()
    };
    let title = merged("title");
    let body = merged("body");
    let category = merged("category");

    // 4. 게시판 파이프라인으로 생성 (훅 발동 + 저장).
    let pipeline = match &state.board {
        Some(p) => p.clone(),
        None => {
            state.record_error("CR0005", "게시판 파이프라인 미기동", "board");
            return axum::response::Json(serde_json::json!({
                "code": "CR0005",
                "message": "게시판 파이프라인 미기동."
            }))
        }
    };
    let create_req = crate::pipeline::CreatePostRequest {
        title: title.clone(),
        body: body.clone(),
        extra: serde_json::json!({ "template_defaults": defaults, "category": category }),
    };
    let post_id = match pipeline.handle_create(create_req).await {
        Ok((status, axum::response::Json(resp))) if status.is_success() => {
            let id = resp.id;
            if id <= 0 {
                state.record_error("CR0005", "인스턴스 생성 실패", "board");
                return axum::response::Json(serde_json::json!({
                    "code": "CR0005",
                    "message": "인스턴스 생성 실패 (id 미반환)."
                }))
            }
            id
        }
        Ok((status, axum::response::Json(err_body))) => {
            let code = err_body.code.clone();
            state.record_error(&code, "인스턴스 생성 실패", "board");
            return axum::response::Json(serde_json::json!({
                "code": code,
                "message": "인스턴스 생성 실패",
                "http_status": status.as_u16()
            }))
        }
        Err((status, axum::response::Json(err_body))) => {
            let code = err_body["code"].as_str().unwrap_or("CJ1201").to_string();
            let msg = err_body["message"].as_str().unwrap_or("").to_string();
            state.record_error(&code, &msg, "board");
            return axum::response::Json(serde_json::json!({
                "code": code,
                "message": msg,
                "http_status": status.as_u16()
            }))
        }
    };

    // 5. 템플릿 바인딩 기록 (template_id/version/sync_policy + payload).
    let payload_fields = defaults.to_string();
    match store
        .apply_template_to_board_post(post_id, &template_id, template.version as i64, &payload_fields, sync_policy)
        .await
    {
        Ok(()) => axum::response::Json(serde_json::json!({
            "code": "OK",
            "id": post_id,
            "template_id": template_id,
            "template_version": template.version,
            "sync_policy": sync_policy
        })),
        Err(e) => {
            state.record_error("CR0005", &e.to_string(), "template");
            axum::response::Json(serde_json::json!({
                "code": "CR0005",
                "message": format!("템플릿 바인딩 기록 실패: {e}")
            }))
        }
    }
}

/// Phase 41 Step 2: 템플릿 인스턴스 생성 쿼리 파라미터.
#[derive(serde::Deserialize)]
struct TemplateInstanceQuery {
    #[serde(default)]
    template_id: Option<String>,
    #[serde(default)]
    tenant: Option<String>,
    #[serde(default)]
    sync_policy: String,
}

impl TemplateInstanceQuery {
    fn tenant(&self) -> String {
        self.tenant.clone().filter(|t| !t.is_empty()).unwrap_or_else(|| "default".to_string())
    }
}

/// Phase 42 P2: 블로그 템플릿 기반 인스턴스 생성.
async fn blog_instance_create(
    state: axum::extract::State<AppState>,
    query: axum::extract::Query<TemplateInstanceQuery>,
    axum::extract::Json(req): axum::extract::Json<Value>,
) -> axum::response::Json<Value> {
    use crate::meta::store::DataStore;
    let template_id = match query.0.template_id.clone() {
        Some(t) if !t.is_empty() => t,
        _ => {
            return axum::response::Json(serde_json::json!({
                "code": "CJ1202",
                "message": "template_id 쿼리 파라미터가 필요합니다."
            }))
        }
    };
    let sync_policy = if query.0.sync_policy.is_empty() { "frozen" } else { &query.0.sync_policy };

    let (store, _cache, _loader) = match state.first_pipeline() {
        Some(p) => p,
        None => {
            return axum::response::Json(serde_json::json!({
                "code": "CR0005",
                "message": "파이프라인이 초기화되지 않았습니다."
            }))
        }
    };
    let tenant = query.0.tenant();

    // 1. 템플릿 활성 버전 조회.
    let template = match store.get_template(&template_id, &tenant).await {
        Ok(Some(t)) => t,
        Ok(None) => {
            return axum::response::Json(serde_json::json!({
                "code": "CR4006",
                "message": format!("템플릿을 찾을 수 없습니다: {template_id}")
            }))
        }
        Err(e) => {
            state.record_error("CR4006", &e.to_string(), "template");
            return axum::response::Json(serde_json::json!({
                "code": "CR0005",
                "message": format!("템플릿 조회 실패: {e}")
            }))
        }
    };

    // 2. 템플릿 fields default 수집.
    let defaults = collect_template_fields(&template.payload);

    // 3. 요청 바디와 병합 (요청값 우선).
    let merged = |key: &str| -> String {
        req.get(key)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| defaults.get(key).and_then(|v| v.as_str()).map(|s| s.to_string()))
            .unwrap_or_default()
    };
    let title = merged("title");
    let body = merged("body");

    // 4. 블로그 파이프라인으로 생성.
    let domain_pipeline = match state.domains.iter().find(|d| d.domain == "blog") {
        Some(p) => p.clone(),
        None => {
            state.record_error("CR0005", "블로그 파이프라인 미기동", "blog");
            return axum::response::Json(serde_json::json!({
                "code": "CR0005",
                "message": "블로그 파이프라인 미기동."
            }))
        }
    };
    let create_payload = serde_json::json!({
        "title": title,
        "body": body,
        "category": merged("category"),
        "extra": defaults
    });
    let result = domain_pipeline.handle_request(create_payload).await;
    let article_id: i64 = match result {
        Ok((status, axum::response::Json(resp))) if status.is_success() => {
            resp.id
        }
        Ok((status, axum::response::Json(resp))) => {
            let code = resp.code.clone();
            let msg = resp.message.clone().unwrap_or_default();
            state.record_error(&code, &msg, "blog");
            return axum::response::Json(serde_json::json!({
                "code": code, "message": msg, "http_status": status.as_u16()
            }))
        }
        Err((status, axum::response::Json(err_body))) => {
            let code = err_body["code"].as_str().unwrap_or("CJ1201").to_string();
            let msg = err_body["message"].as_str().unwrap_or("").to_string();
            state.record_error(&code, &msg, "blog");
            return axum::response::Json(serde_json::json!({
                "code": code, "message": msg, "http_status": status.as_u16()
            }))
        }
    };
    if article_id <= 0 {
        state.record_error("CR0005", "인스턴스 생성 실패", "blog");
        return axum::response::Json(serde_json::json!({
            "code": "CR0005",
            "message": "인스턴스 생성 실패 (id 미반환)."
        }))
    }

    // 5. 템플릿 바인딩 기록.
    let payload_fields = defaults.to_string();
    match store
        .apply_template_to_blog_article(article_id, &template_id, template.version as i64, &payload_fields, sync_policy)
        .await
    {
        Ok(()) => axum::response::Json(serde_json::json!({
            "code": "OK",
            "id": article_id,
            "template_id": template_id,
            "template_version": template.version,
            "sync_policy": sync_policy
        })),
        Err(e) => {
            state.record_error("CR0005", &e.to_string(), "template");
            axum::response::Json(serde_json::json!({
                "code": "CR0005",
                "message": format!("템플릿 바인딩 기록 실패: {e}")
            }))
        }
    }
}

/// Phase 42 P2: 쇼핑몰 템플릿 기반 인스턴스 생성.
async fn shop_instance_create(
    state: axum::extract::State<AppState>,
    query: axum::extract::Query<TemplateInstanceQuery>,
    axum::extract::Json(req): axum::extract::Json<Value>,
) -> axum::response::Json<Value> {
    use crate::meta::store::DataStore;
    let template_id = match query.0.template_id.clone() {
        Some(t) if !t.is_empty() => t,
        _ => {
            return axum::response::Json(serde_json::json!({
                "code": "CJ1202",
                "message": "template_id 쿼리 파라미터가 필요합니다."
            }))
        }
    };
    let sync_policy = if query.0.sync_policy.is_empty() { "frozen" } else { &query.0.sync_policy };

    let (store, _cache, _loader) = match state.first_pipeline() {
        Some(p) => p,
        None => {
            return axum::response::Json(serde_json::json!({
                "code": "CR0005",
                "message": "파이프라인이 초기화되지 않았습니다."
            }))
        }
    };
    let tenant = query.0.tenant();

    // 1. 템플릿 활성 버전 조회.
    let template = match store.get_template(&template_id, &tenant).await {
        Ok(Some(t)) => t,
        Ok(None) => {
            return axum::response::Json(serde_json::json!({
                "code": "CR4006",
                "message": format!("템플릿을 찾을 수 없습니다: {template_id}")
            }))
        }
        Err(e) => {
            state.record_error("CR4006", &e.to_string(), "template");
            return axum::response::Json(serde_json::json!({
                "code": "CR0005",
                "message": format!("템플릿 조회 실패: {e}")
            }))
        }
    };

    // 2. 템플릿 fields default 수집.
    let defaults = collect_template_fields(&template.payload);

    // 3. 요청 바디와 병합 (요청값 우선).
    let merged_str = |key: &str| -> String {
        req.get(key)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| defaults.get(key).and_then(|v| v.as_str()).map(|s| s.to_string()))
            .unwrap_or_default()
    };
    let name = merged_str("name");

    // 4. 상품 직접 생성 (DomainPipeline 경로). shop_products는 DirectHandler 사용.
    let product = crate::meta::store::ShopProduct {
        id: 0,
        name: name.clone(),
        price: req.get("price").and_then(|p| p.as_f64())
            .or_else(|| defaults.get("price").and_then(|v| v.as_f64()))
            .unwrap_or(0.0),
        stock: req.get("stock").and_then(|s| s.as_i64())
            .or_else(|| defaults.get("stock").and_then(|v| v.as_i64()))
            .unwrap_or(0) as i32,
        category: merged_str("category"),
        slug: merged_str("slug"),
        extra: serde_json::to_value(&defaults).unwrap_or(serde_json::Value::Null),
        created_at: crate::http::now_ts(),
        tenant_id: tenant.clone(),
        template_id: String::new(),
        template_version: 0,
        sync_policy: "frozen".into(),
    };
    let product_id = match store.save_shop_product(&product).await {
        Ok(id) => id,
        Err(e) => {
            state.record_error("CR0005", &e.to_string(), "shop");
            return axum::response::Json(serde_json::json!({
                "code": "CR0005",
                "message": format!("상품 저장 실패: {e}")
            }))
        }
    };

    // 5. 템플릿 바인딩 기록.
    let payload_fields = defaults.to_string();
    match store
        .apply_template_to_shop_product(product_id, &template_id, template.version as i64, &payload_fields, sync_policy)
        .await
    {
        Ok(()) => axum::response::Json(serde_json::json!({
            "code": "OK",
            "id": product_id,
            "template_id": template_id,
            "template_version": template.version,
            "sync_policy": sync_policy
        })),
        Err(e) => {
            state.record_error("CR0005", &e.to_string(), "template");
            axum::response::Json(serde_json::json!({
                "code": "CR0005",
                "message": format!("템플릿 바인딩 기록 실패: {e}")
            }))
        }
    }
}

/// Phase 41 Step 2: 템플릿 활성 버전 +1 (개선 버전 생성).
async fn admin_template_bump(
    state: axum::extract::State<AppState>,
    axum::extract::Path(template_id): axum::extract::Path<String>,
    query: axum::extract::Query<TemplateQuery>,
) -> axum::response::Json<Value> {
    use crate::meta::store::DataStore;
    let (store, cache, _loader) = match state.first_pipeline() {
        Some(p) => p,
        None => {
            return axum::response::Json(serde_json::json!({
                "code": "CR0005",
                "message": "파이프라인이 초기화되지 않았습니다."
            }))
        }
    };
    let tenant = query.0.tenant();
    match store.bump_template_version(&template_id, &tenant).await {
        Ok(new_version) => {
            // Phase 42 P5: 캐시 무효화.
            cache.template_cache_invalidate(&tenant, &format!("detail:{}", template_id)).await;
            cache.template_cache_invalidate(&tenant, &format!("tree:{}", template_id)).await;
            cache.template_publish_invalidation(&tenant, &template_id).await;
            axum::response::Json(serde_json::json!({
                "code": "OK",
                "template_id": template_id,
                "new_version": new_version
            }))
        },
        Err(e) => {
            state.record_error("CR0005", &e.to_string(), "template");
            axum::response::Json(serde_json::json!({
                "code": "CR4006",
                "message": format!("템플릿 버전 증분 실패: {e}")
            }))
        }
    }
}

/// Phase 41 Step 2: 템플릿 payload를 sync_policy='live' 인스턴스에 재적용 (개선 전파).
async fn admin_template_propagate(
    state: axum::extract::State<AppState>,
    axum::extract::Path(template_id): axum::extract::Path<String>,
    query: axum::extract::Query<TemplateQuery>,
) -> axum::response::Json<Value> {
    use crate::meta::store::DataStore;
    let (store, _cache, _loader) = match state.first_pipeline() {
        Some(p) => p,
        None => {
            return axum::response::Json(serde_json::json!({
                "code": "CR0005",
                "message": "파이프라인이 초기화되지 않았습니다."
            }))
        }
    };
    let tenant = query.0.tenant();
    // 활성 템플릿 payload의 fields default 수집.
    let template = match store.get_template(&template_id, &tenant).await {
        Ok(Some(t)) => t,
        Ok(None) => {
            return axum::response::Json(serde_json::json!({
                "code": "CR4006",
                "message": format!("템플릿을 찾을 수 없습니다: {template_id}")
            }))
        }
        Err(e) => {
            state.record_error("CR0005", &e.to_string(), "template");
            return axum::response::Json(serde_json::json!({
                "code": "CR0005",
                "message": format!("템플릿 조회 실패: {e}")
            }))
        }
    };
    let payload_fields = collect_template_fields(&template.payload).to_string();
    let domain = &query.0.domain; // "" = all, "board", "blog", "shop"
    let result = match domain.as_str() {
        "board" => store.propagate_template_to_board_posts(&tenant, &template_id, &payload_fields).await,
        "blog" => store.propagate_template_to_blog_articles(&tenant, &template_id, &payload_fields).await,
        "shop" => store.propagate_template_to_shop_products(&tenant, &template_id, &payload_fields).await,
        _ => store.propagate_template_to_all_domains(&tenant, &template_id, &payload_fields).await,
    };
    match result {
        Ok(count) => axum::response::Json(serde_json::json!({
            "code": "OK",
            "template_id": template_id,
            "domain": if domain.is_empty() { "all" } else { domain },
            "propagated": count
        })),
        Err(e) => {
            state.record_error("CR0005", &e.to_string(), "template");
            axum::response::Json(serde_json::json!({
                "code": "CR0005",
                "message": format!("개선 전파 실패: {e}")
            }))
        }
    }
}

/// Phase 41 Step 2: 사이트 템플릿 프로비저닝 (테넌트별 body 노드 복사).
async fn admin_apply_site_template(
    state: axum::extract::State<AppState>,
    axum::extract::Path(tenant_id): axum::extract::Path<String>,
    query: axum::extract::Query<SiteTemplateQuery>,
) -> axum::response::Json<Value> {
    use crate::meta::store::DataStore;
    let (store, _cache, _loader) = match state.first_pipeline() {
        Some(p) => p,
        None => {
            return axum::response::Json(serde_json::json!({
                "code": "CR0005",
                "message": "파이프라인이 초기화되지 않았습니다."
            }))
        }
    };
    let site_template_id = match &query.0.site_template_id {
        Some(t) if !t.is_empty() => t.clone(),
        _ => {
            return axum::response::Json(serde_json::json!({
                "code": "CJ1202",
                "message": "site_template_id 쿼리 파라미터가 필요합니다."
            }))
        }
    };
    match store.apply_site_template(&tenant_id, &site_template_id).await {
        Ok(count) => axum::response::Json(serde_json::json!({
            "code": "OK",
            "tenant_id": tenant_id,
            "site_template_id": site_template_id,
            "copied": count
        })),
        Err(e) => {
            state.record_error("CR0005", &e.to_string(), "template");
            axum::response::Json(serde_json::json!({
                "code": "CR0005",
                "message": format!("사이트 템플릿 프로비저닝 실패: {e}")
            }))
        }
    }
}

#[derive(serde::Deserialize)]
struct SiteTemplateQuery {
    #[serde(default)]
    site_template_id: Option<String>,
}

/// Phase 40: 모듈 활성화/비활성화 토글.
///
/// `enabled`을 반전하여 L3에 저장하고 L1 캐시를 무효화한다.
async fn admin_module_toggle(
    state: axum::extract::State<AppState>,
    axum::extract::Path(module_id): axum::extract::Path<String>,
) -> axum::response::Json<Value> {
    use crate::meta::store::DataStore;
    let (store, cache, _loader) = match state.first_pipeline() {
        Some(p) => p,
        None => {
            return axum::response::Json(serde_json::json!({
                "code": "CR0005",
                "message": "파이프라인이 초기화되지 않았습니다."
            }))
        }
    };
    let existing = match store.get_module_raw(&module_id).await {
        Ok(Some(m)) => m,
        Ok(None) => {
            return axum::response::Json(serde_json::json!({
                "code": "CR4005",
                "message": format!("모듈 '{module_id}'을(를) 찾을 수 없습니다.")
            }))
        }
        Err(e) => {
            state.record_error("CR0005", &e.to_string(), "admin_module");
            return axum::response::Json(serde_json::json!({
                "code": "CR0005",
                "message": format!("모듈 조회 실패: {e}")
            }))
        }
    };
    let new_enabled = !existing.enabled;
    let updated = crate::meta::store::MetaModule {
        enabled: new_enabled,
        ..existing.clone()
    };
    match store.upsert_module(&updated).await {
        Ok(()) => {
            cache.invalidate(&module_id).await;
            axum::response::Json(serde_json::json!({
                "code": "OK",
                "module_id": module_id,
                "enabled": new_enabled,
                "message": if new_enabled { "활성화됨" } else { "비활성화됨" }
            }))
        }
        Err(e) => {
            state.record_error("CR0005", &e.to_string(), "admin_module");
            axum::response::Json(serde_json::json!({
                "code": "CR0005",
                "message": format!("모듈 토글 실패: {e}")
            }))
        }
    }
}

/// Phase 40: 모듈 핫-리로드 (L1 캐시 무효화 → 재로드).
async fn admin_module_reload(
    state: axum::extract::State<AppState>,
    axum::extract::Path(module_id): axum::extract::Path<String>,
) -> axum::response::Json<Value> {
    let (_store, cache, _loader) = match state.first_pipeline() {
        Some(p) => p,
        None => {
            return axum::response::Json(serde_json::json!({
                "code": "CR0005",
                "message": "파이프라인이 초기화되지 않았습니다."
            }))
        }
    };
    cache.invalidate(&module_id).await;
    axum::response::Json(serde_json::json!({
        "code": "OK",
        "module_id": module_id,
        "message": format!("모듈 '{module_id}' L1 캐시 무효화 완료 (다음 요청 시 재로드)")
    }))
}

/// Phase 40: 확장 스크립트 배포 (컴파일 검증 → L3 저장 → L1 무효화).
///
/// 요청 바디: `{ "module_id", "script_source", "kind"?, "base_module_id"?, "enabled"? }`
/// 에러: CJ1101(구문 오류), CR0005(DB 실패).
async fn admin_module_deploy(
    state: axum::extract::State<AppState>,
    axum::Json(body): axum::Json<Value>,
) -> axum::response::Json<Value> {
    use crate::meta::store::DataStore;
    let module_id = body
        .get("module_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_default();
    if module_id.is_empty() {
        return axum::response::Json(serde_json::json!({
            "code": "CJ1201",
            "message": "module_id는 필수입니다."
        }));
    }
    let script_source = body
        .get("script_source")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_default();
    if script_source.is_empty() {
        return axum::response::Json(serde_json::json!({
            "code": "CJ1201",
            "message": "script_source는 필수입니다."
        }));
    }
    let (store, cache, loader) = match state.first_pipeline() {
        Some(p) => p,
        None => {
            return axum::response::Json(serde_json::json!({
                "code": "CR0005",
                "message": "파이프라인이 초기화되지 않았습니다."
            }))
        }
    };

    // 1단계: 스크립트 컴파일 검증 (CJ1101).
    if let Err(e) = loader.compile_validate(&script_source) {
        state.record_error("CJ1101", &e.to_string(), "admin_module_deploy");
        return axum::response::Json(serde_json::json!({
            "code": "CJ1101",
            "message": format!("스크립트 구문 오류: {e}")
        }));
    }

    // 기존 버전/종류를 보존(존재할 때), 없으면 기본값.
    let (kind, base_module_id, version, enabled) = match store
        .get_module_raw(&module_id)
        .await
    {
        Ok(Some(prev)) => (
            prev.kind,
            prev.base_module_id,
            prev.version + 1,
            prev.enabled,
        ),
        _ => (
            body.get("kind").and_then(|v| v.as_str()).unwrap_or("extension").to_string(),
            body.get("base_module_id").and_then(|v| v.as_str()).map(String::from),
            1,
            body.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true),
        ),
    };

    let module = crate::meta::store::MetaModule {
        module_id: module_id.clone(),
        kind,
        base_module_id,
        script_source: script_source.clone(),
        content_hash: format!("{module_id}_h{version}"),
        version,
        enabled,
    };

    match store.upsert_module(&module).await {
        Ok(()) => {
            cache.invalidate(&module_id).await;
            axum::response::Json(serde_json::json!({
                "code": "OK",
                "module_id": module_id,
                "version": version,
                "enabled": enabled,
                "message": format!("모듈 '{module_id}' v{version} 배포 완료")
            }))
        }
        Err(e) => {
            state.record_error("CR0005", &e.to_string(), "admin_module_deploy");
            axum::response::Json(serde_json::json!({
                "code": "CR0005",
                "message": format!("모듈 배포 실패: {e}")
            }))
        }
    }
}

/// Phase 23: 서버 ping (가벼운 생존 확인).
async fn admin_ping(state: axum::extract::State<AppState>) -> axum::response::Json<Value> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    axum::response::Json(serde_json::json!({
        "code": "OK",
        "ts": now
    }))
}

/// Phase 28: OpenAPI spec JSON 서빙.
///
/// `docs/openapi.json` 파일을 서빙. 파일 없으면 기본 skeleton 반환.
async fn openapi_json_handler() -> axum::response::Response {
    let json_spec = std::fs::read_to_string("docs/openapi.json")
        .or_else(|_| std::fs::read_to_string("../docs/openapi.json"))
        .ok();
    let spec_str = json_spec.unwrap_or_else(|| {
        "{\"openapi\":\"3.0.3\",\"info\":{\"title\":\"Meta Server API\",\"version\":\"1.0.0\"},\"paths\":{}}"
            .to_string()
    });
    axum::response::Response::builder()
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(spec_str))
        .unwrap()
}

/// Phase 28: Swagger UI (swagger-ui-dist CDN).
async fn swagger_ui_handler() -> axum::response::Response {
    let html = r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <title>Meta Server API — Swagger UI</title>
  <link rel="stylesheet" href="https://unpkg.com/swagger-ui-dist@5/swagger-ui.css">
  <style>body{margin:0;background:#0f1117}</style>
</head>
<body>
  <div id="swagger-ui"></div>
  <script src="https://unpkg.com/swagger-ui-dist@5/swagger-ui-bundle.js"></script>
  <script>
    window.onload = function() {
      const ui = SwaggerUIBundle({
        url: '/openapi.json',
        dom_id: '#swagger-ui',
        deepLinking: true,
        presets: [SwaggerUIBundle.presets.apis],
        layout: 'StandaloneLayout',
        supportedSubmitMethods: ['get','post','put','delete']
      });
    };
  </script>
</body>
</html>"#;
    axum::response::Response::builder()
        .header(axum::http::header::CONTENT_TYPE, "text/html")
        .body(axum::body::Body::from(html.to_string()))
        .unwrap()
}

/// Phase 27: 서버 상태 즉시 확인 (health check trigger).
///
/// DB backend + Redis 상태 확인 후 반환.
async fn admin_ping_trigger(state: axum::extract::State<AppState>) -> axum::response::Json<Value> {
    let start = std::time::Instant::now();
    let db_ok = state.db_backend != "memory" || state.board.is_some();
    let db_latency = start.elapsed().as_millis() as u64;

    axum::response::Json(serde_json::json!({
        "code": "OK",
        "status": "ok",
        "db": {
            "ok": db_ok,
            "backend": state.db_backend,
            "latency_ms": db_latency
        },
        "redis": {
            "enabled": state.redis_enabled,
            "ok": state.redis_enabled
        },
        "ts": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }))
}

/// Phase 27: metrics + status + error logs를 단일 JSON로 export.
async fn admin_export(
    state: axum::extract::State<AppState>,
) -> axum::response::Response {
    let usage = state.engine.memory_usage();
    let uptime = crate::admin::uptime_seconds(&state.started_at);
    let l1_warm = if let Some(b) = &state.board {
        b.cache.len().await
    } else {
        0
    };

    let m = &state.metrics;
    use std::sync::atomic::Ordering;
    let board = m.http_requests.board.load(Ordering::SeqCst);
    let erp = m.http_requests.erp.load(Ordering::SeqCst);
    let blog = m.http_requests.blog.load(Ordering::SeqCst);
    let shop = m.http_requests.shop.load(Ordering::SeqCst);
    let cache_hits = m.cache_hits.l1.load(Ordering::SeqCst)
        + m.cache_hits.l2.load(Ordering::SeqCst)
        + m.cache_hits.l3.load(Ordering::SeqCst);
    let cache_misses = m.cache_misses.l1.load(Ordering::SeqCst)
        + m.cache_misses.l2.load(Ordering::SeqCst)
        + m.cache_misses.l3.load(Ordering::SeqCst);
    let hook_exec = m.hook_executions.load(Ordering::SeqCst);
    let hook_err = m.hook_errors.load(Ordering::SeqCst);

    let error_logs: Vec<Value> = state
        .error_logs
        .lock()
        .unwrap()
        .iter()
        .rev()
        .take(100)
        .map(|e| {
            serde_json::json!({
                "code": e.code,
                "domain": e.domain,
                "message": e.message,
                "timestamp": e.timestamp
            })
        })
        .collect();

    let export = serde_json::json!({
        "exported_at": crate::admin::now_iso(),
        "status": {
            "uptime_seconds": uptime,
            "engine": {
                "memory_usage_mb": usage.malloc_size as f64 / 1024.0 / 1024.0,
                "memory_limit_mb": state.engine.memory_limit() / 1024 / 1024
            },
            "database": {
                "backend": state.db_backend,
                "l1_warm_modules": l1_warm
            },
            "redis": {
                "enabled": state.redis_enabled
            },
            "socket": {
                "active_sessions": state.socket_pool.len()
            }
        },
        "metrics": {
            "http_requests": {
                "board": board,
                "erp": erp,
                "blog": blog,
                "shop": shop
            },
            "cache": {
                "hits": cache_hits,
                "misses": cache_misses
            },
            "hooks": {
                "executions": hook_exec,
                "errors": hook_err
            }
        },
        "error_logs": error_logs
    });

    let filename = format!(
        "meta_server_metrics_{}.json",
        chrono::Utc::now().format("%Y%m%d_%H%M%S")
    );
    axum::response::Response::builder()
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .header(
            axum::http::header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", filename),
        )
        .body(axum::body::Body::from(export.to_string()))
        .unwrap()
}

/// 서버 라우터 구성.
/// Phase 32: Request ID 미들웨어.
///
/// 각 HTTP 요청에 고유 request_id를 부여하고:
/// - 응답 헤더 `X-Request-Id`에 포함
/// - request 시작/종료 logging (method, path, status, duration, request_id)
async fn request_id_middleware(
    state: axum::extract::State<AppState>,
    req: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> axum::http::Response<axum::body::Body> {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    // Simple pseudo-random from timestamp bits.
    let rand = (now_ms.wrapping_mul(2654435761) >> 16) % 0xFFFF;
    let request_id = format!("req-{:013x}-{:04x}", now_ms & 0xFFFFFFFFFFFFFFFF, rand);

    let method = req.method().clone();
    let path = req.uri().path().to_string();

    // Phase 35: SLO 계산을 위해 총 요청 카운터 증가
    state.metrics.total_requests.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

    let start = std::time::Instant::now();

    eprintln!(
        "[http] request start: {} {} request_id={}",
        method, path, request_id
    );

    let mut resp = next.run(req).await;

    let duration_ms = start.elapsed().as_millis() as f64 / 1000.0;
    let status = resp.status();

    // Phase 35: 5xx 에러 카운터 증가
    if status.is_server_error() {
        state.metrics.total_errors.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }

    eprintln!(
        "[http] request end: {} {} status={} duration_ms={:.2} request_id={}",
        method, path, status.as_u16(), duration_ms, request_id
    );

    let _ = resp.headers_mut().insert(
        axum::http::HeaderName::from_static("x-request-id"),
        request_id
            .parse()
            .unwrap_or_else(|_| axum::http::HeaderValue::from_static("req-unknown")),
    );
    resp
}

pub fn build_router(
    engine: Engine,
    board: Option<Arc<BoardPipeline>>,
    domains: Vec<Arc<DomainPipeline>>,
    socket_config: SocketConfig,
    socket_script: String,
    db_backend: String,
    migrations: Vec<String>,
    redis_enabled: bool,
    jwt_secret: String,
    jwt_disabled: bool,
    rate_limit_per_minute: u32,
) -> Router {
    let socket_pool = std::sync::Arc::new(crate::socket::session::SessionPool::new(
        &socket_config,
    ));
    let socket_handler = crate::socket::handler::SocketHandler::new(engine.clone(), socket_script);
    let metrics = std::sync::Arc::new(crate::metrics::Metrics::new());
    let rate_limiter = std::sync::Arc::new(crate::rate_limit::RateLimiter::new(
        crate::rate_limit::RateLimitConfig {
            per_minute: rate_limit_per_minute,
            per_domain: std::collections::HashMap::new(),
            user_per_minute: 60,
        },
    ));
    // Phase 39: 서킷 브레이커 (환경변수 구성).
    let cb_threshold: u32 = std::env::var("CIRCUIT_FAILURE_THRESHOLD")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);
    let cb_cooldown_ms: u64 = std::env::var("CIRCUIT_COOLDOWN_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(30_000);
    let circuit_breaker = std::sync::Arc::new(tokio::sync::RwLock::new(
        crate::rate_limit::CircuitBreaker::new(
            cb_threshold,
            std::time::Duration::from_millis(cb_cooldown_ms),
        ),
    ));
    let state = AppState {
        engine,
        board,
        domains,
        socket_pool,
        socket_handler,
        db_backend,
        migrations,
        redis_enabled,
        jwt_secret,
        jwt_disabled,
        metrics: metrics.clone(),
        rate_limiter: rate_limiter.clone(),
        circuit_breaker: circuit_breaker.clone(),
        started_at: SystemTime::now(),
        error_logs: std::sync::Arc::new(std::sync::Mutex::new(VecDeque::new())),
    };

    let mut router = Router::new()
        .route("/openapi.json", get(openapi_json_handler))
        .route("/swagger", get(swagger_ui_handler))
        .route("/health", get(health))
        .route("/health/deep", get(health_deep))
        .route("/metrics", get(metrics_handler))
        .route("/api/engine/info", get(engine_info))
        .route("/api/engine/eval", axum::routing::post(eval_probe))
        .route("/api/board", axum::routing::post(board_create))
        .route("/api/erp/purchase-orders", axum::routing::post(domain_create_erp))
        .route("/api/blog/articles", axum::routing::post(domain_create_blog).get(list_blog_articles))
        .route("/api/shop/orders", axum::routing::post(domain_create_shop))
        .route("/api/tags", axum::routing::post(create_tag).get(list_tags_by_post))
        .route("/api/blog/comments", axum::routing::post(create_comment).get(list_comments_by_article))
        .route("/api/tenant/posts", get(list_posts_by_tenant))
        .route(
            "/api/shop/products",
            axum::routing::post(create_shop_product).get(list_shop_products),
        )
        .route(
            "/api/shop/carts",
            axum::routing::post(add_to_cart).get(list_carts_by_customer),
        )
        .route("/ws", get(crate::socket::ws::ws_handler))
        .route("/admin", get(admin_dashboard))
        .route("/api/admin/status", get(admin_status))
        .route("/api/admin/metrics", get(admin_metrics))
        .route("/api/admin/logs", get(admin_logs))
        .route("/api/admin/cache/flush", axum::routing::post(admin_cache_flush))
        .route("/api/admin/ping", get(admin_ping))
        .route("/api/admin/ping/trigger", axum::routing::post(admin_ping_trigger))
        .route("/api/admin/export", get(admin_export))
        // Phase 41: 계층형 템플릿 (읽기 API)
        .route("/api/templates", get(template_list))
        .route("/api/templates/:id", get(template_get))
        .route("/api/templates/:id/tree", get(template_tree))
        // Phase 41 Step 2: 템플릿 기반 인스턴스 생성 (게시판)
        .route("/api/board/items", axum::routing::post(board_instance_create))
        // Phase 42 P2: Blog/Shop 템플릿 기반 인스턴스 생성
        .route("/api/blog/items", axum::routing::post(blog_instance_create))
        .route("/api/shop/items", axum::routing::post(shop_instance_create))
        // Phase 41 Step 2: Admin 템플릿 개선 전파
        .route("/api/admin/templates/:id/bump", axum::routing::post(admin_template_bump))
        .route("/api/admin/templates/:id/propagate", axum::routing::post(admin_template_propagate))
        .route("/api/admin/tenants/:id/apply-site-template", axum::routing::post(admin_apply_site_template))
        // Phase 40: Admin Extension (모듈) Management
        .route("/api/admin/modules", get(admin_modules_list))
        .route("/api/admin/modules/deploy", axum::routing::post(admin_module_deploy))
        .route("/api/admin/modules/:id", get(admin_module_get))
        .route("/api/admin/modules/:id/toggle", axum::routing::post(admin_module_toggle))
        .route("/api/admin/modules/:id/reload", axum::routing::post(admin_module_reload))
        .route("/ws/admin", get(crate::socket::ws::ws_admin_handler));

    // Phase 14: JWT 인증 미들웨어 적용
    let router = router.layer(axum::middleware::from_fn_with_state(
        state.clone(),
        crate::auth::jwt_middleware,
    ));

    // Phase 15: Rate Limiting 미들웨어 적용
    let router = router.layer(axum::middleware::from_fn_with_state(
        state.clone(),
        crate::rate_limit::rate_limit_middleware,
    ));

    // Phase 25: Admin 인증 미들웨어 적용
    let router = router.layer(axum::middleware::from_fn(
        crate::auth::admin_auth_middleware,
    ));

    // Phase 25: CORS 미들웨어 적용 (가장 먼저 실행되어야 하므로 last layer)
    let router = router.layer(axum::middleware::from_fn(
        crate::auth::cors_middleware,
    ));

    // Phase 32: Request ID 미들웨어 적용 (가장 바깥)
    let router = router.layer(axum::middleware::from_fn_with_state(
        state.clone(),
        request_id_middleware,
    ));

    router.with_state(state)
}

/// 게시판 글 생성 핸들러 (`POST /api/board`).
async fn board_create(
    state: axum::extract::State<AppState>,
    axum::extract::Json(req): axum::extract::Json<CreatePostRequest>,
) -> axum::response::Response {
    match &state.board {
        Some(pipeline) => match pipeline.handle_create(req).await {
            Ok((status, Json(body))) => (
                status,
                axum::response::Json(body),
            )
            .into_response(),
            Err((status, Json(body))) => {
                // Phase 24: 에러 로그 기록
                let code = body["code"].as_str().unwrap_or("UNKNOWN").to_string();
                let msg = body["message"].as_str().unwrap_or("").to_string();
                state.record_error(&code, &msg, "board");
                state.metrics.record_hook_error();
                (
                    status,
                    axum::response::Json(body),
                )
                .into_response()
            }
        },
        None => {
            // Phase 24: 에러 로그 기록
            state.record_error("CR0005", "게시판 파이프라인 미기동", "board");
            axum::response::Json(serde_json::json!({
                "code": "CR0005",
                "message": "게시판 파이프라인 미기동 (DATABASE_BACKEND=memory 필요)."
            }))
            .into_response()
        }
    }
}

/// 도메인별 생성 핸들러 — 경로 기반.
///
/// 각 도메인별 별도 핸들러를 만들어 경로와 도메인을 매핑.
async fn domain_create_erp(
    state: axum::extract::State<AppState>,
    axum::extract::Json(payload): axum::extract::Json<Value>,
) -> axum::response::Response {
    handle_domain(&state, "erp", payload).await
}

async fn domain_create_blog(
    state: axum::extract::State<AppState>,
    axum::extract::Json(payload): axum::extract::Json<Value>,
) -> axum::response::Response {
    handle_domain(&state, "blog", payload).await
}

async fn domain_create_shop(
    state: axum::extract::State<AppState>,
    axum::extract::Json(payload): axum::extract::Json<Value>,
) -> axum::response::Response {
    handle_domain(&state, "shop", payload).await
}

async fn handle_domain(
    state: &AppState,
    domain: &str,
    payload: Value,
) -> axum::response::Response {
    match state.domains.iter().find(|d| d.domain == domain) {
        Some(pipeline) => match pipeline.handle_request(payload).await {
            Ok((status, Json(body))) => (status, axum::response::Json(body)).into_response(),
            Err((status, Json(body))) => {
                // Phase 24: 에러 로그 기록
                let code = body["code"].as_str().unwrap_or("UNKNOWN").to_string();
                let msg = body["message"].as_str().unwrap_or("").to_string();
                state.record_error(&code, &msg, domain);
                state.metrics.record_hook_error();
                (status, axum::response::Json(body)).into_response()
            }
        },
        None => {
            // Phase 24: 에러 로그 기록
            state.record_error("CR0005", "도메인 파이프라인 미기동", domain);
            axum::response::Json(serde_json::json!({
                "code": "CR0005",
                "message": "도메인 파이프라인 미기동."
            }))
            .into_response()
        }
    }
}

/// Phase 42: 블로그 글 목록 조회 (GET /api/blog/articles).
async fn list_blog_articles(
    state: axum::extract::State<AppState>,
) -> axum::response::Json<Value> {
    use crate::meta::store::DataStore;
    let store = match state.first_store() {
        Some(s) => s,
        None => {
            return axum::response::Json(serde_json::json!({
                "code": "CR0005",
                "message": "스토어가 초기화되지 않았습니다."
            }))
        }
    };
    let tenant = "default".to_string();
    match store.list_blog_articles_for_tenant(&tenant).await {
        Ok(articles) => axum::response::Json(serde_json::json!({
            "code": "OK",
            "count": articles.len(),
            "articles": articles
        })),
        Err(e) => {
            state.record_error("CR0005", &e.to_string(), "blog");
            axum::response::Json(serde_json::json!({
                "code": "CR4005",
                "message": format!("블로그 글 목록 조회 실패: {e}")
            }))
        }
    }
}

/// Phase 36: 태그 생성 (POST /api/tags).
async fn create_tag(
    state: axum::extract::State<AppState>,
    axum::extract::Json(payload): axum::extract::Json<Value>,
) -> axum::response::Response {
    use crate::meta::store::BoardTag;
    let store = match state.first_store() {
        Some(s) => s,
        None => {
            return axum::response::Json(serde_json::json!({
                "code": "CR0005",
                "message": "데이터 스토어 미기동."
            }))
            .into_response()
        }
    };
    let board_id = payload["board_id"].as_str().unwrap_or("board").to_string();
    let post_id = payload["post_id"].as_i64().unwrap_or(0);
    let tag = payload["tag"].as_str().unwrap_or("").trim().to_string();
    if post_id <= 0 || tag.is_empty() {
        return axum::response::Json(serde_json::json!({
            "code": "CJ1201",
            "message": "post_id > 0, tag 필요."
        }))
        .into_response();
    }
    let entity = BoardTag { id: 0, board_id, post_id, tag, created_at: 0, tenant_id: "default".into() };
    match store.create_board_tag(&entity).await {
        Ok(id) => {
            let tags = store.list_board_tags_by_post(post_id).await.ok().unwrap_or_default();
            axum::response::Json(serde_json::json!({
                "id": id, "post_id": post_id, "tags": tags
            })).into_response()
        }
        Err(e) => {
            state.record_error("CJ1102", &e.to_string(), "tag");
            axum::response::Json(serde_json::json!({
                "code": "CJ1102",
                "message": format!("태그 저장 실패: {e}")
            })).into_response()
        }
    }
}

/// Phase 36: 게시글별 태그 조회 (GET /api/tags?post_id=1).
async fn list_tags_by_post(
    state: axum::extract::State<AppState>,
    req: axum::http::Request<axum::body::Body>,
) -> axum::response::Response {
    let store = match state.first_store() {
        Some(s) => s,
        None => {
            return axum::response::Json(serde_json::json!({
                "code": "CR0005",
                "message": "데이터 스토어 미기동."
            })).into_response()
        }
    };
    let post_id = AppState::query_param_i64(&req, "post_id");
    match store.list_board_tags_by_post(post_id).await {
        Ok(tags) => axum::response::Json(serde_json::json!({
            "post_id": post_id, "tags": tags
        })).into_response(),
        Err(e) => {
            state.record_error("CJ1102", &e.to_string(), "tag");
            axum::response::Json(serde_json::json!({
                "code": "CJ1102",
                "message": format!("태그 조회 실패: {e}")
            })).into_response()
        }
    }
}

/// Phase 36: 블로그 댓글 생성 (POST /api/blog/comments).
async fn create_comment(
    state: axum::extract::State<AppState>,
    axum::extract::Json(payload): axum::extract::Json<Value>,
) -> axum::response::Response {
    use crate::meta::store::BlogComment;
    let store = match state.first_store() {
        Some(s) => s,
        None => {
            return axum::response::Json(serde_json::json!({
                "code": "CR0005",
                "message": "데이터 스토어 미기동."
            })).into_response()
        }
    };
    let article_id = payload["article_id"].as_i64().unwrap_or(0);
    let author = payload["author"].as_str().unwrap_or("").trim().to_string();
    let body = payload["body"].as_str().unwrap_or("").trim().to_string();
    let parent_id = payload["parent_id"].as_i64();
    if article_id <= 0 || author.is_empty() || body.is_empty() {
        return axum::response::Json(serde_json::json!({
            "code": "CJ1201",
            "message": "article_id > 0, author, body 필요."
        })).into_response();
    }
    let entity = BlogComment { id: 0, article_id, author, body, parent_id, created_at: 0, tenant_id: "default".into() };
    match store.create_blog_comment(&entity).await {
        Ok(id) => {
            let comments = store.list_blog_comments_by_article(article_id).await.ok().unwrap_or_default();
            axum::response::Json(serde_json::json!({
                "id": id, "article_id": article_id, "comments": comments
            })).into_response()
        }
        Err(e) => {
            state.record_error("CJ1102", &e.to_string(), "comment");
            axum::response::Json(serde_json::json!({
                "code": "CJ1102",
                "message": format!("댓글 저장 실패: {e}")
            })).into_response()
        }
    }
}

/// Phase 36: 블로그 글별 댓글 조회 (GET /api/blog/comments?article_id=1).
async fn list_comments_by_article(
    state: axum::extract::State<AppState>,
    req: axum::http::Request<axum::body::Body>,
) -> axum::response::Response {
    let store = match state.first_store() {
        Some(s) => s,
        None => {
            return axum::response::Json(serde_json::json!({
                "code": "CR0005",
                "message": "데이터 스토어 미기동."
            })).into_response()
        }
    };
    let article_id = AppState::query_param_i64(&req, "article_id");
    match store.list_blog_comments_by_article(article_id).await {
        Ok(comments) => axum::response::Json(serde_json::json!({
            "article_id": article_id, "comments": comments
        })).into_response(),
        Err(e) => {
            state.record_error("CJ1102", &e.to_string(), "comment");
            axum::response::Json(serde_json::json!({
                "code": "CJ1102",
                "message": format!("댓글 조회 실패: {e}")
            })).into_response()
        }
    }
}

/// Phase 37: tenant별 게시판 글 목록 (GET /api/tenant/posts?tenant=acme).
async fn list_posts_by_tenant(
    state: axum::extract::State<AppState>,
    req: axum::http::Request<axum::body::Body>,
) -> axum::response::Response {
    let store = match state.first_store() {
        Some(s) => s,
        None => {
            return axum::response::Json(serde_json::json!({
                "code": "CR0005",
                "message": "데이터 스토어 미기동."
            })).into_response()
        }
    };
    // tenant: query ?tenant= 우선, 없으면 X-Tenant-Id 헤더, 없으면 default
    let query_tenant: Option<String> = req
        .uri()
        .query()
        .unwrap_or("")
        .split('&')
        .find_map(|pair| pair.split_once('='))
        .filter(|(k, _)| *k == "tenant")
        .map(|(_, v)| v.to_string());
    let tenant = match query_tenant {
        Some(t) if !t.is_empty() => t,
        _ => req
            .headers()
            .get("x-tenant-id")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("default")
            .to_string(),
    };
    match store.list_board_posts_for_tenant(&tenant).await {
        Ok(posts) => axum::response::Json(serde_json::json!({
            "tenant": tenant, "count": posts.len(), "posts": posts
        })).into_response(),
        Err(e) => {
            state.record_error("CJ1102", &e.to_string(), "tenant");
            axum::response::Json(serde_json::json!({
                "code": "CJ1102",
                "message": format!("tenant 조회 실패: {e}")
            })).into_response()
        }
    }
}

// ---------- Phase 38: Shop API ----------

/// 상품 생성 (POST /api/shop/products).
async fn create_shop_product(
    state: axum::extract::State<AppState>,
    req: axum::http::Request<axum::body::Body>,
) -> axum::response::Response {
    let store = match state.first_store() {
        Some(s) => s,
        None => {
            return axum::response::Json(serde_json::json!({
                "code": "CR0005", "message": "데이터 스토어 미기동."
            })).into_response()
        }
    };
    let body = match axum::body::to_bytes(req.into_body(), 256 * 1024).await {
        Ok(b) => b,
        Err(_) => {
            state.record_error("CR0003", "payload too large", "shop_product");
            return axum::response::Json(serde_json::json!({
                "code": "CR0003", "message": "페이로드 크기 초과."
            })).into_response()
        }
    };
    let json: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => {
            state.record_error("CJ1101", &e.to_string(), "shop_product");
            return axum::response::Json(serde_json::json!({
                "code": "CJ1101", "message": format!("잘못된 JSON: {e}")
            })).into_response()
        }
    };
    let name = json.get("name").and_then(|n| n.as_str()).unwrap_or("");
    if name.is_empty() {
        state.record_error("CJ1103", "name required", "shop_product");
        return axum::response::Json(serde_json::json!({
            "code": "CJ1103", "message": "name 필수."
        })).into_response()
    }
    let product = ShopProduct {
        id: 0,
        name: name.to_string(),
        price: json.get("price").and_then(|p| p.as_f64()).unwrap_or(0.0),
        stock: json.get("stock").and_then(|s| s.as_i64()).unwrap_or(0) as i32,
        category: json.get("category").and_then(|c| c.as_str()).unwrap_or("").to_string(),
        slug: json.get("slug").and_then(|s| s.as_str()).unwrap_or("").to_string(),
        extra: json.get("extra").cloned().unwrap_or(serde_json::Value::Null),
        created_at: crate::http::now_ts(),
        tenant_id: "default".into(),
        template_id: String::new(),
        template_version: 0,
        sync_policy: String::from("frozen"),
    };
    match store.save_shop_product(&product).await {
        Ok(id) => axum::response::Json(serde_json::json!({
            "code": "OK", "id": id, "name": product.name
        })).into_response(),
        Err(e) => {
            state.record_error("CR0005", &e.to_string(), "shop_product");
            axum::response::Json(serde_json::json!({
                "code": "CR0005", "message": format!("상품 저장 실패: {e}")
            })).into_response()
        }
    }
}

/// 상품 목록 (GET /api/shop/products?tenant=default).
async fn list_shop_products(
    state: axum::extract::State<AppState>,
    req: axum::http::Request<axum::body::Body>,
) -> axum::response::Response {
    let store = match state.first_store() {
        Some(s) => s,
        None => {
            return axum::response::Json(serde_json::json!({
                "code": "CR0005", "message": "데이터 스토어 미기동."
            })).into_response()
        }
    };
    let tenant = req
        .uri()
        .query()
        .unwrap_or("")
        .split('&')
        .find_map(|p| p.split_once('='))
        .filter(|(k, _)| *k == "tenant")
        .map(|(_, v)| v.to_string())
        .unwrap_or_else(|| "default".into());
    match store.list_shop_products_for_tenant(&tenant).await {
        Ok(products) => axum::response::Json(serde_json::json!({
            "tenant": tenant, "count": products.len(), "products": products
        })).into_response(),
        Err(e) => {
            state.record_error("CJ1102", &e.to_string(), "shop_product");
            axum::response::Json(serde_json::json!({
                "code": "CJ1102", "message": format!("상품 목록 조회 실패: {e}")
            })).into_response()
        }
    }
}

/// 카트에 담기 (POST /api/shop/carts).
async fn add_to_cart(
    state: axum::extract::State<AppState>,
    req: axum::http::Request<axum::body::Body>,
) -> axum::response::Response {
    let store = match state.first_store() {
        Some(s) => s,
        None => {
            return axum::response::Json(serde_json::json!({
                "code": "CR0005", "message": "데이터 스토어 미기동."
            })).into_response()
        }
    };
    let body = match axum::body::to_bytes(req.into_body(), 256 * 1024).await {
        Ok(b) => b,
        Err(_) => {
            return axum::response::Json(serde_json::json!({
                "code": "CR0003", "message": "페이로드 크기 초과."
            })).into_response()
        }
    };
    let json: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => {
            state.record_error("CJ1101", &e.to_string(), "shop_cart");
            return axum::response::Json(serde_json::json!({
                "code": "CJ1101", "message": format!("잘못된 JSON: {e}")
            })).into_response()
        }
    };
    let customer = json.get("customer").and_then(|c| c.as_str()).unwrap_or("");
    if customer.is_empty() {
        state.record_error("CJ1103", "customer required", "shop_cart");
        return axum::response::Json(serde_json::json!({
            "code": "CJ1103", "message": "customer 필수."
        })).into_response()
    }
    let product_id = json.get("product_id").and_then(|p| p.as_i64()).unwrap_or(0);
    if product_id <= 0 {
        state.record_error("CJ1103", "product_id required", "shop_cart");
        return axum::response::Json(serde_json::json!({
            "code": "CJ1103", "message": "product_id 필수."
        })).into_response()
    }
    let cart = ShopCart {
        id: 0,
        customer: customer.to_string(),
        product_id,
        quantity: json.get("quantity").and_then(|q| q.as_i64()).unwrap_or(1) as i32,
        extra: json.get("extra").cloned().unwrap_or(serde_json::Value::Null),
        created_at: crate::http::now_ts(),
        tenant_id: "default".into(),
    };
    match store.save_shop_cart(&cart).await {
        Ok(id) => axum::response::Json(serde_json::json!({
            "code": "OK", "id": id, "customer": cart.customer
        })).into_response(),
        Err(e) => {
            state.record_error("CR0005", &e.to_string(), "shop_cart");
            axum::response::Json(serde_json::json!({
                "code": "CR0005", "message": format!("카트 저장 실패: {e}")
            })).into_response()
        }
    }
}

/// 고객별 카트 목록 (GET /api/shop/carts?customer=abc).
async fn list_carts_by_customer(
    state: axum::extract::State<AppState>,
    req: axum::http::Request<axum::body::Body>,
) -> axum::response::Response {
    let store = match state.first_store() {
        Some(s) => s,
        None => {
            return axum::response::Json(serde_json::json!({
                "code": "CR0005", "message": "데이터 스토어 미기동."
            })).into_response()
        }
    };
    let customer = req
        .uri()
        .query()
        .unwrap_or("")
        .split('&')
        .find_map(|p| p.split_once('='))
        .filter(|(k, _)| *k == "customer")
        .map(|(_, v)| v.to_string())
        .unwrap_or_default();
    if customer.is_empty() {
        return axum::response::Json(serde_json::json!({
            "code": "CJ1103", "message": "customer 필수."
        })).into_response()
    }
    match store.list_shop_carts_by_customer(&customer).await {
        Ok(carts) => axum::response::Json(serde_json::json!({
            "customer": customer, "count": carts.len(), "carts": carts
        })).into_response(),
        Err(e) => {
            state.record_error("CJ1102", &e.to_string(), "shop_cart");
            axum::response::Json(serde_json::json!({
                "code": "CJ1102", "message": format!("카트 조회 실패: {e}")
            })).into_response()
        }
    }
}
