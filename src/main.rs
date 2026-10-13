//! Axum 서버 기동 + rquickjs 엔진 초기화.
//!
//! Phase 5: PostgreSQL(기본) + Redis(선택적) + 게시판 파이프라인 + 소켓 통신.
//!
//! 환경변수:
//! - `DATABASE_BACKEND`: "postgres" | "memory" (기본 "postgres")
//! - `DATABASE_URL`: PostgreSQL 연결 문자열
//! - `REDIS_ENABLED`: "true" | "false" (기본 "false")
//! - `REDIS_URL`: Redis 연결 문자열 (기본 "redis://localhost:6379")
//! - `SOCKET_MAX_SESSIONS`: 최대 동시 소켓 세션 수 (기본 1000)

use std::net::SocketAddr;
use std::sync::Arc;

use tokio::net::TcpListener;

use meta_server::domain::DomainPipeline;
use meta_server::engine::{Engine, ThrottleConfig};
use meta_server::http::build_router;
use meta_server::meta::store::{create_data_store, meta_store_config_from_env, DataStore};
use meta_server::meta::redis::{create_redis_store, RedisLikeStore};
use meta_server::observability::{EventQueueObserver, Observability};
use meta_server::pipeline::{BoardPipeline, build_board_pipeline_with_store};

/// Phase 33: Config Validation — 기동 전 환경변수 검증.
///
/// 잘못된 값이면 명확한 에러 메시지와 함께 종료.
fn validate_config() -> Result<(), Box<dyn std::error::Error>> {
    // DATABASE_BACKEND 유효값 확인
    let backend = std::env::var("DATABASE_BACKEND").unwrap_or_else(|_| "postgres".to_string());
    if backend != "memory" && backend != "postgres" {
        return Err(format!(
            "CR0004: DATABASE_BACKEND='{}' is invalid. Use 'memory' or 'postgres'.", backend
        ).into());
    }

    // REDIS_ENABLED 유효값 확인
    let redis_enabled = std::env::var("REDIS_ENABLED").unwrap_or_else(|_| "false".to_string());
    if !matches!(redis_enabled.as_str(), "true" | "false" | "1" | "0") {
        return Err(format!(
            "CR0005: REDIS_ENABLED='{}' is invalid. Use 'true' or 'false'.", redis_enabled
        ).into());
    }

    // JWT_SECRET 길이 확인 (disabled 아니면 32자+)
    let jwt_disabled = std::env::var("JWT_DISABLED")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);
    if !jwt_disabled {
        let secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| "dev-secret-change-me".to_string());
        if secret.len() < 32 {
            eprintln!(
                "[MetaServer] WARNING: JWT_SECRET is {} chars (recommended: 32+). Using dev default.",
                secret.len()
            );
        }
    }

    // RATE_LIMIT_PER_MIN 음수 확인
    let rate = std::env::var("RATE_LIMIT_PER_MIN").ok().and_then(|v| v.parse::<i64>().ok());
    if let Some(r) = rate {
        if r < 0 {
            return Err(format!(
                "CR0006: RATE_LIMIT_PER_MIN={} is negative. Must be >= 0.", r
            ).into());
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Phase 33: Config Validation — 기동 전 환경변수 검증
    validate_config()?;

    // 1. rquickjs 엔진 초기화 (Throttling 강제)
    let cfg = ThrottleConfig::default();
    let engine = Engine::new(cfg.clone()).expect("CR0002: rquickjs 엔진 초기화 실패");

    let usage = engine.memory_usage();
    println!(
        "[MetaServer] rquickjs engine initialized \
         (memory_limit={} bytes, cpu_timeout_ms={}, malloc_limit={} malloc_size={})",
        engine.memory_limit(),
        engine.cpu_timeout_ms(),
        usage.malloc_limit,
        usage.malloc_size
    );

    // Phase 34: Engine warm-up — rquickjs JIT prewarm (첫 요청 latency 감소)
    if let Err(e) = engine.warm_up() {
        eprintln!("[MetaServer] WARNING: engine warm-up 실패: {e}");
    } else {
        println!("[MetaServer] rquickjs engine warmed up");
    }

    // 2. 메타스토어 설정 (환경변수)
    let store_cfg = meta_store_config_from_env();
    println!(
        "[MetaServer] MetaStore: backend={}, postgres_url={}, redis_enabled={}",
        store_cfg.backend, store_cfg.postgres_url, store_cfg.redis.enabled
    );

    // 3. DataStore(L3) 초기화: PostgreSQL(기본) 또는 메모리 스텁
    let store: Arc<dyn DataStore> = match create_data_store(&store_cfg).await {
        Ok(s) => {
            println!("[MetaServer] L3 DataStore: {} connected", store_cfg.backend);
            Arc::from(s)
        }
        Err(e) => {
            eprintln!("[MetaServer] L3 DataStore 연결 실패: {e}");
            eprintln!("[MetaServer] 메모리 스텁으로 폴백합니다. (DATABASE_BACKEND=memory)");
            // PostgreSQL 실패 시 메모리 스텁으로 폴백
            let mem_cfg = meta_server::meta::store::MetaStoreConfig {
                backend: "memory".to_string(),
                ..store_cfg.clone()
            };
            let s = create_data_store(&mem_cfg).await?;
            Arc::from(s)
        }
    };

    // 3.5. Phase 41: 파일 채널 템플릿 시드 (templates/*.json → meta_templates)
    match meta_server::meta::store::seed_templates_from_files(store.as_ref()).await {
        Ok(n) if n > 0 => println!("[MetaServer] 템플릿 파일 시드: {n}개"),
        Ok(_) => {}
        Err(e) => eprintln!("[MetaServer] 템플릿 파일 시드 실패(계속): {e}"),
    }

    // 3.5.1. 마이그레이션 (Phase 13)
    let migrations: Vec<String> = match store_cfg.backend.as_str() {
        "postgres" => {
            // PostgresStore에서 이미 connect 시 마이그레이션 적용
            vec!["001_init.sql".to_string(), "002_domain_tables.sql".to_string()]
        }
        _ => vec![],
    };

    // 4. Redis(L2) 초기화: 실제 Redis 또는 InMemoryPubSub 스텁
    let redis_store: Arc<dyn RedisLikeStore> = match create_redis_store(&store_cfg.redis).await {
        Ok(rs) => {
            if store_cfg.redis.enabled {
                println!("[MetaServer] L2 Redis: connected ({})", store_cfg.redis.url);
            } else {
                println!("[MetaServer] L2 Redis: InMemoryPubSub 스텁 (REDIS_ENABLED=false)");
            }
            rs
        }
        Err(e) => {
            eprintln!("[MetaServer] L2 Redis 연결 실패: {e}");
            eprintln!("[MetaServer] InMemoryPubSub 스텁으로 폴백합니다.");
            let fallback_cfg = meta_server::meta::redis::RedisConfig {
                enabled: false,
                ..store_cfg.redis.clone()
            };
            create_redis_store(&fallback_cfg).await.unwrap()
        }
    };

    // 5. 옵저버 + 게시판 파이프라인
    let (event_obs, mut event_rx) = EventQueueObserver::new(500, 256);
    let observability = Arc::new(Observability::new(Arc::new(event_obs), 500));

    // 옵저버 이벤트 소비자
    tokio::spawn(async move {
        while let Some(ev) = event_rx.recv().await {
            match ev {
                meta_server::observability::EvalEvent::Start { module_id, version } => {
                    println!("[OBS] start {module_id} v{version}");
                }
                meta_server::observability::EvalEvent::End { module_id, report } => {
                    let _ = module_id;
                    if report.is_slow(500) {
                        println!(
                            "[OBS] SLOW {} elapsed_ms={}",
                            report.module_id,
                            report.elapsed.as_millis()
                        );
                    }
                }
                meta_server::observability::EvalEvent::Error {
                    module_id,
                    code,
                    message,
                } => {
                    println!("[OBS] ERROR {module_id} {code}: {message}");
                }
                meta_server::observability::EvalEvent::Slow {
                    module_id,
                    elapsed_ms,
                    threshold_ms,
                } => {
                    println!(
                        "[OBS] SLOW {module_id} {elapsed_ms}ms (threshold {threshold_ms}ms)"
                    );
                }
            }
        }
    });

    // 6. 게시판 파이프라인 (L3 DataStore 기반)
    let board: Option<Arc<BoardPipeline>> = match build_board_pipeline_with_store(
        &engine,
        store.clone(),
        redis_store.clone(),
        observability.clone(),
    )
    .await
    {
        Ok(p) => {
            println!("[MetaServer] board pipeline initialized");
            Some(Arc::new(p))
        }
        Err(e) => {
            println!("[MetaServer] board pipeline init failed: {e}");
            None
        }
    };

    // 6.5. 도메인별 파이프라인 (Phase 12: ERP, Blog, Shop)
    let mut domains: Vec<Arc<DomainPipeline>> = Vec::new();

    // ERP
    let erp_base = r#"
        var api = {};
        api.before_process = function(payload) {
            if (!payload.workflow_id) {
                throw new Error(JSON.stringify({code: "EJ1201", message: "워크플로우 ID 필수"}));
            }
            return payload;
        };
        api.after_process = function(event) { return event; };
    "#;
    let erp_ext = r#"
        api.before_process = function(payload) {
            var validated = Base.before_process(payload);
            if (payload.quantity <= 0) {
                throw new Error(JSON.stringify({code: "EJ1202", message: "수량은 1 이상"}));
            }
            if (!payload.supplier) {
                throw new Error(JSON.stringify({code: "EJ1203", message: "공급자 필수"}));
            }
            return validated;
        };
        api.before_delete = function() {
            throw new Error(JSON.stringify({code: "EJ1401", message: "발주서 삭제 불가"}));
        };
    "#;
    if let Ok(p) = DomainPipeline::build(
        &engine,
        store.clone(),
        redis_store.clone(),
        observability.clone(),
        "erp",
        "base.erp",
        "ext.erp_po",
        erp_base,
        erp_ext,
    )
    .await
    {
        println!("[MetaServer] erp pipeline initialized");
        domains.push(Arc::new(p));
    }

    // Blog
    let blog_base = r#"
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
    let blog_ext = r#"
        api.before_publish = function(payload) {
            var validated = Base.before_publish(payload);
            if (typeof validated.languages === "undefined") {
                validated.languages = ["general"];
            }
            return validated;
        };
    "#;
    if let Ok(p) = DomainPipeline::build(
        &engine,
        store.clone(),
        redis_store.clone(),
        observability.clone(),
        "blog",
        "base.blog",
        "ext.blog_tech",
        blog_base,
        blog_ext,
    )
    .await
    {
        println!("[MetaServer] blog pipeline initialized");
        domains.push(Arc::new(p));
    }

    // Shop
    let shop_base = r#"
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
    let shop_ext = r#"
        api.before_order = function(payload) {
            var validated = Base.before_order(payload);
            if (payload.quantity <= 0) {
                throw new Error(JSON.stringify({code: "SJ1403", message: "수량 1 이상"}));
            }
            if (typeof validated.warranty_months === "undefined") {
                validated.warranty_months = 12;
            }
            return validated;
        };
    "#;
    if let Ok(p) = DomainPipeline::build(
        &engine,
        store.clone(),
        redis_store.clone(),
        observability.clone(),
        "shop",
        "base.shop",
        "ext.shop_electronics",
        shop_base,
        shop_ext,
    )
    .await
    {
        println!("[MetaServer] shop pipeline initialized");
        domains.push(Arc::new(p));
    }

    // 7. Redis Pub/Sub 구독 루프 기동 (L1 무효화)
    if let Some(board) = &board {
        let cache_clone = board.cache.clone();
        let _ = redis_store
            .subscribe_invalidation(Box::new(move |token| {
                // L1 캐시 무효화 (비동기: spawn)
                let c = cache_clone.clone();
                tokio::spawn(async move {
                    c.invalidate(&token.module_id).await;
                    println!("[L1] 무효화: {} v{}", token.module_id, token.version);
                });
            }))
            .await;
    }

    // 7.5. Phase 26: Cache Warming — 기동 시 모든 모듈을 L1에 사전 적재
    if let Some(board) = &board {
        let all_modules: &[&str] = &[
            "base.board", "ext.notice",
            "base.erp", "ext.erp_po",
            "base.blog", "ext.blog_tech",
            "base.shop", "ext.shop_electronics",
        ];
        board.cache.warmup(all_modules).await;
    } else if !domains.is_empty() {
        let all_modules: &[&str] = &[
            "base.board", "ext.notice",
            "base.erp", "ext.erp_po",
            "base.blog", "ext.blog_tech",
            "base.shop", "ext.shop_electronics",
        ];
        domains[0].cache.warmup(all_modules).await;
    }

    // 8. 소켓 통신 레이어 (Phase 5)
    let socket_config = meta_server::socket::SocketConfig::from_env();
    let socket_script = std::fs::read_to_string("scripts/socket_hooks.js").unwrap_or_else(|_| {
        "".to_string()
    });
    println!(
        "[MetaServer] socket layer: max_sessions={}",
        socket_config.max_sessions
    );

    // Phase 14: JWT 설정
    let jwt_config = meta_server::auth::JwtConfig::from_env();
    println!(
        "[MetaServer] JWT: secret={} disabled={}",
        if jwt_config.secret == "dev-secret-change-me" {
            "<default>"
        } else {
            "<custom>"
        },
        jwt_config.disabled
    );

    // Phase 15: Rate Limiting 설정
    let rate_limit_config = meta_server::rate_limit::RateLimitConfig::from_env();
    println!(
        "[MetaServer] Rate Limiting: {} requests/minute",
        rate_limit_config.per_minute
    );

    let router = build_router(
        engine,
        board,
        domains,
        socket_config,
        socket_script,
        store_cfg.backend.clone(),
        migrations,
        store_cfg.redis.enabled,
        jwt_config.secret,
        jwt_config.disabled,
        rate_limit_config.per_minute,
    );

    let addr: SocketAddr = "0.0.0.0:3000".parse()?;
    let listener = TcpListener::bind(addr).await?;
    println!("[MetaServer] listening on http://{addr}");

    // Phase 33: Graceful Shutdown — SIGTERM/SIGINT 수신 시 in-flight 요청 완료 후 종료
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    println!("[MetaServer] graceful shutdown complete");
    Ok(())
}

/// Phase 33: shutdown 신호 대기 (SIGINT + SIGTERM).
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("CR0003: SIGINT listener init fail");
    };

    #[cfg(unix)]
    let terminate = async {
        let mut sig = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("CR0003: SIGTERM listener init fail");
        sig.recv().await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => println!("[MetaServer] SIGINT received, initiating graceful shutdown"),
        _ = terminate => println!("[MetaServer] SIGTERM received, initiating graceful shutdown"),
    }
}
