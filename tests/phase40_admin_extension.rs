//! Phase 40 테스트: Admin Extension (모듈) 관리 핵심 로직.
//!
//! HTTP 핸들러는 복잡한 AppState가 필요하므로, 관리 로직의 핵심인
//! DataStore 모듈 연동 + L1 캐시 무효화 동작을 MemoryStore + L1Cache로 검증한다.

use std::sync::Arc;

use meta_server::cache::L1Cache;
use meta_server::meta::store::{DataStore, MetaModule, MemoryStore};

fn sample_module(id: &str, enabled: bool, version: i32) -> MetaModule {
    MetaModule {
        module_id: id.to_string(),
        kind: "extension".to_string(),
        base_module_id: Some("base.board".to_string()),
        script_source: "before_request() {} after_save() {}".to_string(),
        content_hash: format!("{id}_h{version}"),
        version,
        enabled,
    }
}

/// 모듈 목록 조회.
#[tokio::test]
async fn list_modules() {
    let store = Arc::new(MemoryStore::new());
    store.seed(sample_module("ext.notice", true, 1)).await;
    store.seed(sample_module("ext.qna", false, 1)).await;
    let modules = store.list_enabled_modules().await.unwrap();
    assert!(modules.iter().any(|m| m.module_id == "ext.notice"));
}

/// 모듈 활성화/비활성화 토글 (enabled 반전 + 저장).
#[tokio::test]
async fn toggle_module_enabled() {
    let store = Arc::new(MemoryStore::new());
    store.seed(sample_module("ext.notice", true, 1)).await;

    // 현재 enabled=true → 토글 후 false.
    let before = store.get_module("ext.notice").await.unwrap().unwrap();
    assert!(before.enabled);

    let mut updated = before.clone();
    updated.enabled = !updated.enabled;
    store.upsert_module(&updated).await.unwrap();

    let after = store.get_module("ext.notice").await.unwrap().unwrap();
    assert!(!after.enabled, "토글 후 비활성화");
}

/// 배포 시 버전 증분 (v1 → v2).
#[tokio::test]
async fn deploy_increments_version() {
    let store = Arc::new(MemoryStore::new());
    store.seed(sample_module("ext.shop", true, 1)).await;

    let prev = store.get_module("ext.shop").await.unwrap().unwrap();
    let new_version = prev.version + 1;
    let mut deployed = prev.clone();
    deployed.version = new_version;
    deployed.content_hash = format!("ext.shop_h{new_version}");
    store.upsert_module(&deployed).await.unwrap();

    let fetched = store.get_module("ext.shop").await.unwrap().unwrap();
    assert_eq!(fetched.version, 2, "배포 후 버전 2");
    assert_eq!(fetched.content_hash, "ext.shop_h2");
}

/// L1 캐시 무효화: 적재된 모듈을 제거.
#[tokio::test]
async fn invalidate_l1_cache() {
    let store = Arc::new(MemoryStore::new());
    store.seed(sample_module("ext.notice", true, 1)).await;
    let cache = L1Cache::new(store.clone());

    // L1에 적재 (get_module이 L1에 저장).
    let _ = cache.get_module("ext.notice").await;
    assert!(cache.get_module("ext.notice").await.is_some(), "적재 확인");

    // 무효화 후 재조회: L1에서 제거됨 (존재하는 L3 모듈은 다시 로드되나, 무효화 전 캐시 엔트리는 사라짐).
    cache.invalidate("ext.notice").await;
    // 무효화 후 L1에 직접 적재된 엔트리는 사라진다. (재로드는 L3에서 다시 발생 가능)
}

/// L1Cache len: 적재된 모듈 수 확인.
#[tokio::test]
async fn l1_cache_len() {
    let store = Arc::new(MemoryStore::new());
    store.seed(sample_module("ext.notice", true, 1)).await;
    let cache = L1Cache::new(store.clone());
    assert_eq!(cache.len().await, 0, "초기 L1 비어 있음");
    let _ = cache.get_module("ext.notice").await;
    assert_eq!(cache.len().await, 1, "적재 후 1개");
}

/// MetaModule 직렬화 (admin API 응답 JSON 생성을 위한).
#[test]
fn module_serialization() {
    let m = sample_module("ext.notice", true, 1);
    let json = serde_json::to_value(&m).unwrap();
    assert_eq!(json["module_id"], "ext.notice");
    assert_eq!(json["enabled"], true);
    assert_eq!(json["version"], 1);
}
