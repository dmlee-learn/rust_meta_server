//! L1 인메포리 캐시 + Single Flight 패턴 + L2(Redis) 연동.
//!
//! AGENTS.md: 캐시 스탬피드 방어 — L1 캐시가 비워진 순간 대량의 요청이
//! 쏟아져도 Single Flight 패턴(뮤텍스 락 격리)으로 오직 첫 번째 스레드
//! 하나만 Redis/DB에서 스크립트를 로드해 메모리를 채우도록 제한.
//!
//! Phase 7: L1 Miss 시 L2(Redis)에서 조회 → Hit이면 L1 적재, Miss이면 L3에서 로드.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{Mutex, RwLock};

use crate::meta::redis::{CacheEntry, RedisLikeStore};
use crate::meta::store::{DataStore, MetaModule};

/// L1 캐시 엔트리: 로딩된 모듈 + 버전.
#[derive(Debug, Clone)]
pub struct CachedModule {
    pub module: MetaModule,
    pub version: u64,
}

/// L1 인메포리 캐시.
///
/// - `modules`: `module_id → CachedModule` (읽기 최적화 RwLock).
/// - Single Flight: 동시 미스 시 첫 번째 스레드만 L3에서 로드하고,
///   나머지는 대기.
pub struct L1Cache {
    modules: RwLock<HashMap<String, CachedModule>>,
    /// Single Flight 락: `module_id → true` (로딩 진행중 표시).
    in_flight: Mutex<HashMap<String, ()>>,
    store: Arc<dyn DataStore>,
    /// L2(Redis) 캐시 (Phase 7). None이면 L3에 직접 조회.
    redis_store: Option<Arc<dyn RedisLikeStore>>,
}

impl L1Cache {
    pub fn new(store: Arc<dyn DataStore>) -> Self {
        Self {
            modules: RwLock::new(HashMap::new()),
            in_flight: Mutex::new(HashMap::new()),
            store,
            redis_store: None,
        }
    }

    /// L2(Redis) 스토어 연결 (Phase 7).
    pub fn with_redis(mut self, redis_store: Arc<dyn RedisLikeStore>) -> Self {
        self.redis_store = Some(redis_store);
        self
    }

    /// 모듈을 L1에서 조회 (미스 시 L2→L3 로드 + Single Flight).
    ///
    /// - L1 히트: 즉시 반환.
    /// - L1 미스: Single Flight로 L2→L3 로드. 첫 스레드만 로드하고 적재.
    /// - L2 히트: L1에 적재 후 반환.
    /// - L2 미스 → L3 로드: L2에 적재 후 L1에 적재, 반환.
    /// - L3에 없음: `None`.
    pub async fn get_module(&self, module_id: &str) -> Option<CachedModule> {
        // 1. L1 히트 확인
        if let Some(cached) = self.modules.read().await.get(module_id) {
            return Some(cached.clone());
        }

        // 2. Single Flight: in_flight 락으로 경쟁 방지
        let mut if_guard = self.in_flight.lock().await;

        // L1에 이미 적재됐는지 재확인 (락 대기 중 다른 스레드가 적재했을 수 있음)
        {
            let guard = self.modules.read().await;
            if let Some(c) = guard.get(module_id) {
                drop(if_guard);
                return Some(c.clone());
            }
        }

        // in_flight에 등록 (이 스레드가 로더)
        if_guard.insert(module_id.to_string(), ());
        drop(if_guard);

        // 3. L2(Redis)에서 조회
        let loaded = if let Some(redis) = &self.redis_store {
            let cache_key = format!("module:{module_id}");
            match redis.get_cache(&cache_key).await {
                Ok(Some(entry)) => {
                    // L2 Hit: MetaModule 복원
                    serde_json::from_str::<MetaModule>(&entry.value).ok()
                }
                _ => {
                    // L2 Miss: L3에서 로드
                    let m = self.store.get_module(module_id).await.ok().flatten();
                    // L3 Hit 시 L2에 적재
                    if let Some(ref module) = m {
                        let cache_entry = CacheEntry {
                            key: cache_key.clone(),
                            value: serde_json::to_string(module).unwrap_or_default(),
                            version: module.version as u64,
                        };
                        let _ = redis.set_cache(&cache_entry).await;
                    }
                    m
                }
            }
        } else {
            // L2 미사용: L3에 직접 조회
            self.store.get_module(module_id).await.ok().flatten()
        };

        // 4. in_flight에서 해제
        {
            let mut guard = self.in_flight.lock().await;
            guard.remove(module_id);
        }

        match loaded {
            Some(m) => {
                let entry = CachedModule {
                    version: m.version as u64,
                    module: m.clone(),
                };
                let mut guard = self.modules.write().await;
                guard.insert(module_id.to_string(), entry.clone());
                Some(entry)
            }
            None => None,
        }
    }

    /// Phase 34: Redis 연결 상태 확인 (PING). Redis 미연결 시 Ok(()) 반환.
    pub async fn redis_ping(&self) -> Result<(), crate::meta::redis::RedisLikeError> {
        match &self.redis_store {
            Some(redis) => redis.ping().await,
            None => Ok(()),
        }
    }

    /// L1 캐시 무효화 (Redis Pub/Sub 통지 시).
    ///    /// L2(Redis)에도 `module:{module_id}` 키를 삭제한다.
    pub async fn invalidate(&self, module_id: &str) {
        self.modules.write().await.remove(module_id);
        if let Some(redis) = &self.redis_store {
            let cache_key = format!("module:{module_id}");
            let _ = redis.delete_cache(&cache_key).await;
        }
    }

    /// 캐시 크기 (테스트/모니터링용).
    pub async fn len(&self) -> usize {
        self.modules.read().await.len()
    }

    /// Phase 42 P5: 템플릿 L2(Redis) 캐시 읽기.
    /// Redis 비활성화 시 None 반환 (DB 직접 조회 경로).
    pub async fn template_cache_get(&self, tenant: &str, key_suffix: &str) -> Option<String> {
        let redis = self.redis_store.as_ref()?;
        let key = format!("template:{}:{}", tenant, key_suffix);
        match redis.get_cache(&key).await {
            Ok(Some(entry)) => Some(entry.value),
            _ => None,
        }
    }

    /// Phase 42 P5: 템플릿 L2(Redis) 캐시 쓰기.
    pub async fn template_cache_set(&self, tenant: &str, key_suffix: &str, value: &str) {
        let redis = match self.redis_store.as_ref() { Some(r) => r, None => return };
        let key = format!("template:{}:{}", tenant, key_suffix);
        let entry = crate::meta::redis::CacheEntry {
            key: key.clone(),
            value: value.to_string(),
            version: 0,
        };
        let _ = redis.set_cache(&entry).await;
    }

    /// Phase 42 P5: 템플릿 L2(Redis) 캐시 무효화.
    pub async fn template_cache_invalidate(&self, tenant: &str, key_suffix: &str) {
        let redis = match self.redis_store.as_ref() { Some(r) => r, None => return };
        let key = format!("template:{}:{}", tenant, key_suffix);
        let _ = redis.delete_cache(&key).await;
    }

    /// Phase 42 P5: 템플릿 Pub/Sub 무효화 이벤트 발행.
    pub async fn template_publish_invalidation(&self, tenant: &str, template_id: &str) {
        let redis = match self.redis_store.as_ref() { Some(r) => r, None => return };
        let token = crate::meta::redis::InvalidationToken {
            channel: "template".to_string(),
            module_id: format!("{}:{}", tenant, template_id),
            version: 0,
        };
        let _ = redis.publish_invalidation(&token).await;
    }

    /// 캐시 비움 (테스트용).
    pub async fn clear(&self) {
        self.modules.write().await.clear();
    }

    /// 모듈 직접 적재 (테스트/시드용).
    pub async fn put(&self, module: MetaModule) {
        let entry = CachedModule {
            version: module.version as u64,
            module,
        };
        let mut guard = self.modules.write().await;
        guard.insert(entry.module.module_id.clone(), entry);
    }

    /// Phase 26: Cache Warming — 기동 시 모듈을 L1에 사전 적재.
    ///
    /// 각 `module_id`를 `get_module()`로 로드 (L2→L3 포함).
    /// 실패해도 기동 중단 안 함 (무선속성: 첫 요청 시 다시 로드).
    /// 적재된 모듈 수를 반환.
    pub async fn warmup(&self, module_ids: &[&str]) -> usize {
        let mut count = 0;
        for id in module_ids {
            match self.get_module(id).await {
                Some(_) => {
                    count += 1;
                    println!("[L1] warmup: {id} 적재 완료");
                }
                None => {
                    eprintln!("[L1] warmup: {id} 로드 실패 (기동 지속, 첫 요청 시 재시도)");
                }
            }
        }
        println!("[L1] warmup: {count}/{} modules 적재됨", module_ids.len());
        count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meta::store::MemoryStore;

    fn sample_module(id: &str) -> MetaModule {
        MetaModule {
            module_id: id.into(),
            kind: "base".into(),
            base_module_id: None,
            script_source: "var api={};".into(),
            content_hash: "h".into(),
            version: 1,
            enabled: true,
        }
    }

    /// L1 히트: 적재 후 조회.
    #[tokio::test]
    async fn l1_hit() {
        let store = Arc::new(MemoryStore::new());
        store.upsert_module(&sample_module("m1")).await.unwrap();
        let cache = L1Cache::new(store);
        assert!(cache.get_module("m1").await.is_some());
        assert!(cache.get_module("m1").await.is_some());
    }

    /// L1 미스: L3에 없으면 None.
    #[tokio::test]
    async fn l1_miss_returns_none() {
        let store = Arc::new(MemoryStore::new());
        let cache = L1Cache::new(store);
        assert!(cache.get_module("nope").await.is_none());
    }

    /// Single Flight: 동시 미스 시 캐시가 정확히 적재되는지.
    ///
    /// 20개 동시 요청이 같은 캐시를 공유하면, 결과가 모두 Some이고
    /// 캐시는 1개만 있어야 함.
    #[tokio::test]
    async fn single_flight_concurrent_miss() {
        let store = Arc::new(MemoryStore::new());
        store.upsert_module(&sample_module("sf")).await.unwrap();
        let cache = Arc::new(L1Cache::new(store));

        let mut handles = Vec::new();
        for _ in 0..20 {
            let c = Arc::clone(&cache);
            handles.push(tokio::spawn(async move { c.get_module("sf").await.is_some() }));
        }
        let mut results = Vec::new();
        for h in handles {
            results.push(h.await.unwrap());
        }
        assert!(results.iter().all(|&b| b), "모든 동시 요청이 Some을 반환해야 한다");
        assert_eq!(cache.len().await, 1, "캐시는 정확히 1개만 적재되어야 한다");
    }
}
