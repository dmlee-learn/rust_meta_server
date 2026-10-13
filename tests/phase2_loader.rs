//! Phase 2 통합 테스트: DataStore + 모듈 로딩 파이프라인 + CJ1101 + Redis 스텁.

use meta_server::engine::loader::LoadError;
use meta_server::engine::{Engine, ModuleLoader, ThrottleConfig};
use meta_server::meta::redis::{CacheEntry, InvalidationToken, RedisConfig, create_redis_store};
use meta_server::meta::store::{
    DataStore, MemoryStore, MetaModule, MetaStoreConfig, create_data_store,
};

fn test_engine() -> Engine {
    Engine::new(ThrottleConfig {
        memory_limit: 16 * 1024 * 1024,
        max_stack: 0,
        cpu_timeout_ms: 1000,
    })
    .unwrap()
}

fn sample_base_module() -> MetaModule {
    let src = r#"
        api.before_create = function(p){ if(!p.title){ throw new Error(JSON.stringify({code:"SJ1201",message:"제목 필수"})); } return p; };
        api.after_save = function(e){ return {saved:true}; };
    "#;
    MetaModule {
        module_id: "base.board".into(),
        kind: "base".into(),
        base_module_id: None,
        script_source: src.into(),
        content_hash: "h1".into(),
        version: 1,
        enabled: true,
    }
}

fn sample_ext_module() -> MetaModule {
    let src = r#"
        api.before_create = function(p){
            var v = Base.before_create(p);
            if(typeof v.pinned === "undefined"){ throw new Error(JSON.stringify({code:"SJ1301",message:"pinned 필수"})); }
            return v;
        };
    "#;
    MetaModule {
        module_id: "ext.notice".into(),
        kind: "extension".into(),
        base_module_id: Some("base.board".into()),
        script_source: src.into(),
        content_hash: "h2".into(),
        version: 1,
        enabled: true,
    }
}

/// MemoryStore 팩토리가 memory backend를 반환하는지.
#[tokio::test]
async fn memory_store_factory() {
    let cfg = MetaStoreConfig {
        backend: "memory".into(),
        postgres_url: String::new(),
        redis: RedisConfig::default(),
    };
    let _store = create_data_store(&cfg).await.unwrap();
}

/// PostgreSQL 팩토리는 인스턴스 없으므로 연결 실패를 반환하는지.
#[tokio::test]
async fn postgres_store_factory_connect_fails_without_instance() {
    let cfg = MetaStoreConfig {
        backend: "postgres".into(),
        postgres_url: "postgres://nouser:nopass@127.0.0.1:59999/nodb".into(),
        redis: RedisConfig::default(),
    };
    // 짧은 타임아웃으로 빠르게 실패
    let res = create_data_store(&cfg).await;
    assert!(
        res.is_err(),
        "인스턴스 없으면 PostgreSQL 연결 실패해야 한다"
    );
}

/// MemoryStore에 모듈 저장 후 조회.
#[tokio::test]
async fn memory_store_upsert_and_get() {
    let store = MemoryStore::new();
    let m = sample_base_module();
    store.upsert_module(&m).await.unwrap();
    let got = store.get_module("base.board").await.unwrap();
    assert!(got.is_some());
    assert_eq!(got.unwrap().module_id, "base.board");
}

/// CJ1101: 구문오류가 있는 스크립트가 LoadError::Syntax로 반환되는지.
#[tokio::test]
async fn syntax_error_detected_as_cj101() {
    let engine = test_engine();
    let loader = ModuleLoader::new(engine, ThrottleConfig::default());
    let bad_src = "function broken( { this is not js }";
    let res: Result<(), LoadError> = loader.compile_validate_public(bad_src);
    assert!(
        matches!(res, Err(LoadError::Syntax(_))),
        "구문오류가 CJ101로 감지되어야 한다"
    );
}

/// 정상 Base+Extension 스크립트가 로드되는지.
#[tokio::test]
async fn valid_base_ext_loads() {
    let store = MemoryStore::new();
    store.upsert_module(&sample_base_module()).await.unwrap();
    store.upsert_module(&sample_ext_module()).await.unwrap();

    let engine = test_engine();
    let loader = ModuleLoader::new(engine, ThrottleConfig::default());
    let modules = loader.load_modules(&store).await.unwrap();
    assert_eq!(modules.len(), 2);
}

/// Base→Extension 바인딩 후 확장 훅이 Base를 상속하는지.
#[tokio::test]
async fn extension_inherits_base_hooks() {
    let base = sample_base_module();
    let ext = sample_ext_module();
    let engine = test_engine();
    let loader = ModuleLoader::new(engine, ThrottleConfig::default());
    let sb = loader.bind_extension(&base, &ext).unwrap();
    // ext 훅 존재 확인
    assert!(loader.has_hook(&sb, "before_create"));
    // Base 훅도 Extension 프로토 타입에서 접근 가능 (상속)
    assert!(loader.has_hook(&sb, "after_save"));
}

/// Base 훅 실행 시 비즈니스 에러(SJ1201)가 스크립트 예외로 전달되는지.
#[tokio::test]
async fn base_hook_business_error_propagates() {
    let base = sample_base_module();
    let ext = sample_ext_module();
    let engine = test_engine();
    let loader = ModuleLoader::new(engine, ThrottleConfig::default());
    let sb = loader.bind_extension(&base, &ext).unwrap();
    // 제목 없이 before_create 호출 -> SJ1201 예외
    let res = sb.eval_i64("Extension.before_create({}); 0");
    assert!(res.is_err(), "제목 없는 payload는 SJ1201로 실패해야 함");
}

/// Redis 스텁: 비활성 시 InMemoryPubSub 반환.
#[tokio::test]
async fn redis_store_disabled_by_default() {
    let cfg = RedisConfig::default();
    assert!(!cfg.enabled);
    let store = create_redis_store(&cfg).await.unwrap();
    // 비활성: InMemoryPubSub 스텁 (Disabled 아님)
    let got = store.get_cache("nope").await.unwrap();
    assert!(got.is_none());
}

/// Redis 스텁: 캐시 읽기/쓰기.
#[tokio::test]
async fn redis_store_cache_read_write() {
    let cfg = RedisConfig {
        enabled: false,
        url: "redis://localhost:6379".into(),
    };
    let store = create_redis_store(&cfg).await.unwrap();
    let entry = CacheEntry {
        key: "mod:base.board".into(),
        value: "v1".into(),
        version: 1,
    };
    store.set_cache(&entry).await.unwrap();
    let got = store.get_cache("mod:base.board").await.unwrap();
    assert!(got.is_some());
    assert_eq!(got.unwrap().version, 1);
}

/// Redis 스텁: Pub/Sub 발신/수신 (in-process).
#[tokio::test]
async fn redis_pubsub_publish_subscribe() {
    let cfg = RedisConfig {
        enabled: true,
        url: "redis://localhost:6379".into(),
    };
    let store = create_redis_store(&cfg).await.unwrap();

    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<InvalidationToken>();
    let mut rx = rx;

    store
        .subscribe_invalidation(Box::new(move |token| {
            let _ = tx.send(token);
        }))
        .await
        .unwrap();

    store
        .publish_invalidation(&InvalidationToken {
            channel: "meta_inval".into(),
            module_id: "base.board".into(),
            version: 2,
        })
        .await
        .unwrap();

    // 비동기 수신 확인 (broadcast delay 허용)
    let mut received = None;
    for _ in 0..50 {
        if let Ok(tok) = rx.try_recv() {
            received = Some(tok);
            break;
        }
        tokio::task::yield_now().await;
    }
    assert!(received.is_some(), "Pub/Sub 수신 실패");
    assert_eq!(received.unwrap().module_id, "base.board");
}
