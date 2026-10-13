//! Phase 3 통합 테스트: 게시판 파이프라인 + CJ1102 무한루프 + 옵저버.
//!
//! Q1-3, Q2-3, Q3-1, Q4-1, Q5-3, Q7-2 검증.

use std::sync::Arc;

use meta_server::cache::L1Cache;
use meta_server::engine::{Engine, ModuleLoader, ThrottleConfig};
use meta_server::meta::store::{MetaModule, MemoryStore};
use meta_server::observability::{EvalEvent, EventQueueObserver, Observability};
use meta_server::pipeline::{BoardPipeline, CreatePostRequest, ModuleLoaderRef};
use meta_server::meta::store::DataStore;

fn sample_base() -> MetaModule {
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

fn sample_ext() -> MetaModule {
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

async fn build_pipeline(cpu_ms: u64) -> BoardPipeline {
    let store = MemoryStore::new();
    store.upsert_module(&sample_base()).await.unwrap();
    store.upsert_module(&sample_ext()).await.unwrap();
    let store_box: Arc<dyn DataStore> = Arc::new(store);

    let engine = Engine::new(ThrottleConfig {
        memory_limit: 32 * 1024 * 1024,
        max_stack: 0,
        cpu_timeout_ms: cpu_ms,
    })
    .unwrap();

    let cache = Arc::new(L1Cache::new(Arc::clone(&store_box)));
    let loader = ModuleLoader::new(engine.clone(), ThrottleConfig {
        memory_limit: 32 * 1024 * 1024,
        max_stack: 0,
        cpu_timeout_ms: cpu_ms,
    });

    BoardPipeline {
        loader: ModuleLoaderRef::new(loader),
        cache,
        store: store_box,
        observability: Arc::new(Observability::noop()),
    }
}

/// 정상 글 생성: title+body 통과, id 반환, 저장 확인.
#[tokio::test]
async fn board_create_success() {
    let pipeline = build_pipeline(0).await;
    let req = CreatePostRequest {
        title: "공지".into(),
        body: "본문".into(),
        extra: serde_json::json!({}),
    };
    let (status, resp) = pipeline.handle_create(req).await.unwrap();
    assert_eq!(status, axum::http::StatusCode::CREATED);
    let body = resp.0;
    assert_eq!(body.code, "OK");
    assert!(body.id > 0, "글 ID가 부여되어야 함");
    assert_eq!(body.title, "공지");
}

/// 제목 없는 글: Input validation으로 CJ1201 차단, 422 반환.
#[tokio::test]
async fn board_create_reject_empty_title() {
    let pipeline = build_pipeline(0).await;
    let req = CreatePostRequest {
        title: "".into(),
        body: "본문".into(),
        extra: serde_json::json!({}),
    };
    let res = pipeline.handle_create(req).await;
    assert!(res.is_err(), "제목 없는 글은 차단되어야 한다");
    let (status, body) = res.unwrap_err();
    assert_eq!(status, axum::http::StatusCode::UNPROCESSABLE_ENTITY, "CJ1201 → 422");
    assert_eq!(body.0.get("code").and_then(|c| c.as_str()), Some("CJ1201"));
}

/// after_save가 저장 후 발동되는지 (저장된 데이터 확인).
#[tokio::test]
async fn board_create_triggers_after_save() {
    let pipeline = build_pipeline(0).await;
    let req = CreatePostRequest {
        title: "확인".into(),
        body: "b".into(),
        extra: serde_json::json!({}),
    };
    let (_, resp) = pipeline.handle_create(req).await.unwrap();
    let id = resp.0.id;
    // 저장된 글이 존재하는지
    let post = pipeline.store.get_board_post(id).await.unwrap();
    assert!(post.is_some(), "after_save 후 글이 저장되어 있어야 함");
    assert_eq!(post.unwrap().title, "확인");
}

/// Q4-1: 무한루프 before_create → CJ1102 (504).
#[tokio::test]
async fn board_create_infinite_loop_cj102() {
    // 무한루프를 before_create에 넣은 확장 모듈
    let mut ext = sample_ext();
    ext.script_source = r#"
        api.before_create = function(p){
            for(let i=0;i<4e9;i++){ }
            return p;
        };
    "#
    .into();

    let store = MemoryStore::new();
    store.upsert_module(&sample_base()).await.unwrap();
    store.upsert_module(&ext).await.unwrap();
    let store_box: Arc<dyn DataStore> = Arc::new(store);

    let engine = Engine::new(ThrottleConfig {
        memory_limit: 32 * 1024 * 1024,
        max_stack: 0,
        cpu_timeout_ms: 200, // 200ms 제한
    })
    .unwrap();

    let cache = Arc::new(L1Cache::new(Arc::clone(&store_box)));
    let loader = ModuleLoader::new(engine.clone(), ThrottleConfig {
        memory_limit: 32 * 1024 * 1024,
        max_stack: 0,
        cpu_timeout_ms: 200,
    });

    let pipeline = BoardPipeline {
        loader: ModuleLoaderRef::new(loader),
        cache,
        store: store_box,
        observability: Arc::new(Observability::noop()),
    };

    let req = CreatePostRequest {
        title: "루프".into(),
        body: "b".into(),
        extra: serde_json::json!({}),
    };
    let res = pipeline.handle_create(req).await;
    assert!(res.is_err(), "무한루프는 CJ1102로 실패해야 한다");
    let (status, body) = res.unwrap_err();
    assert_eq!(
        status,
        axum::http::StatusCode::GATEWAY_TIMEOUT,
        "CJ1102 → 504"
    );
    assert_eq!(body.0.get("code").and_then(|c| c.as_str()), Some("CJ1102"));
}

/// Q5-3/Q7-2: 옵저버가 Start/End/Error 이벤트를 보고하는지.
#[tokio::test]
async fn observer_receives_events() {
    let store = MemoryStore::new();
    store.upsert_module(&sample_base()).await.unwrap();
    store.upsert_module(&sample_ext()).await.unwrap();
    let store_box: Arc<dyn DataStore> = Arc::new(store);

    let engine = Engine::new(ThrottleConfig {
        memory_limit: 32 * 1024 * 1024,
        max_stack: 0,
        cpu_timeout_ms: 0,
    })
    .unwrap();

    let cache = Arc::new(L1Cache::new(Arc::clone(&store_box)));
    let loader = ModuleLoader::new(engine.clone(), ThrottleConfig::default());

    let (obs, mut rx) = EventQueueObserver::new(500, 256);
    let pipeline = BoardPipeline {
        loader: ModuleLoaderRef::new(loader),
        cache,
        store: store_box,
        observability: Arc::new(Observability::new(Arc::new(obs), 500)),
    };

    let req = CreatePostRequest {
        title: "옵저버".into(),
        body: "b".into(),
        extra: serde_json::json!({}),
    };
    let _ = pipeline.handle_create(req).await.unwrap();

    // 이벤트 수집: Start(1) + End(2, before_create+after_save) 최소
    let mut start_count = 0;
    let mut end_count = 0;
    for _ in 0..20 {
        match rx.try_recv() {
            Ok(EvalEvent::Start { .. }) => start_count += 1,
            Ok(EvalEvent::End { .. }) => end_count += 1,
            Ok(EvalEvent::Error { .. }) => {}
            Ok(EvalEvent::Slow { .. }) => {}
            Err(_) => break,
        }
    }
    assert!(start_count >= 1, "최소 1개의 Start 이벤트");
    assert!(end_count >= 1, "최소 1개의 End 이벤트");
}

/// L1 캐시: 모듈이 캐시에 적재되는지.
#[tokio::test]
async fn l1_cache_populated() {
    let pipeline = build_pipeline(0).await;
    assert!(pipeline.cache.get_module("base.board").await.is_some());
    assert!(pipeline.cache.get_module("ext.notice").await.is_some());
    assert!(pipeline.cache.len().await >= 2);
}
