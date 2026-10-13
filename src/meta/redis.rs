//! Redis 기반 L2 글로벌 캐시 + Pub/Sub 동적 갱신기.
//!
//! AGENTS.md의 3단계 데이터 레이어 중 L2(Redis 글로벌 캐시)를 담당한다.
//! Phase 4에서 `redis` crate로 **실제 Redis 인스턴스 연동**을 구현한다.
//!
//! 설정:
//! - `REDIS_ENABLED=true` + `REDIS_URL=redis://localhost:6379` → 실제 Redis
//! - `REDIS_ENABLED=false` → `InMemoryPubSub` 스텁 (기본값, PostgreSQL 우선)
//!
//! Pub/Sub 채널: `meta:invalidation`
//! 메시지 형식: JSON `{"module_id": "base.board", "version": 2, "ts": 1234567890}`
//!
//! L1 캐시 무효화 흐름:
//! 1. 어드민이 `upsert_module` 호출 → L3(PG) update
//! 2. `publish_invalidation(token)` → Redis PUBLISH `meta:invalidation`
//! 3. 모든 인스턴스의 `subscribe_invalidation` 루프가 수신 → L1 캐시 무효화

use std::collections::HashMap;
use std::sync::Arc;

use redis::aio::MultiplexedConnection;
use redis::{AsyncCommands, Client};
use futures::StreamExt;
use tokio::sync::{RwLock, broadcast};

/// Redis(L2) 활성화 설정.
#[derive(Debug, Clone)]
pub struct RedisConfig {
    /// 기본값 false (PostgreSQL 기본, Redis 추후 활성화).
    pub enabled: bool,
    /// Redis 연결 문자열 (활성화 시).
    pub url: String,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            url: "redis://localhost:6379".to_string(),
        }
    }
}

/// Pub/Sub 채널 이름.
pub const INVALIDATION_CHANNEL: &str = "meta:invalidation";

/// 캐시 레코드 (L2).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CacheEntry {
    pub key: String,
    pub value: String,
    pub version: u64,
}

/// Pub/Sub 이벤트 (메타데이터 무효화 통지).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InvalidationToken {
    pub channel: String,
    pub module_id: String,
    pub version: u64,
}

/// L2 캐시 + Pub/Sub 추상 트레잇.
///
/// Phase 3에서 L1(Axum 인메모리)과 이 L2를 연결하고,
/// Redis 인스턴스 활성화 시 `RealRedisStore` 구현체로 교체한다.
#[async_trait::async_trait]
pub trait RedisLikeStore: Send + Sync {
    /// Phase 34: 연결 상태 확인 (PING).
    async fn ping(&self) -> Result<(), RedisLikeError>;

    /// 캐시 읽기. 히트 시 Some, 미스 시 None.
    async fn get_cache(&self, key: &str) -> Result<Option<CacheEntry>, RedisLikeError>;

    /// 캐시 쓰기 (Write-Through).
    async fn set_cache(&self, entry: &CacheEntry) -> Result<(), RedisLikeError>;

    /// 캐시 키 삭제 (무효화 시 L2에서 모듈 제거).
    async fn delete_cache(&self, key: &str) -> Result<(), RedisLikeError>;

    /// 무효화 이벤트 발행 (L1 동적 동기화 통지).
    async fn publish_invalidation(&self, token: &InvalidationToken) -> Result<(), RedisLikeError>;

    /// 무효화 이벤트 구독 (L1 갱신 트리거).
    ///
    /// `handler`는 각 이벤트에 대해 호출된다.
    async fn subscribe_invalidation(
        &self,
        handler: Box<dyn FnMut(InvalidationToken) + Send>,
    ) -> Result<(), RedisLikeError>;
}

#[derive(Debug, thiserror::Error)]
pub enum RedisLikeError {
    #[error("캐시 접근 실패: {0}")]
    Cache(String),
    #[error("Pub/Sub 오류: {0}")]
    PubSub(String),
    #[error("Redis 연결 실패: {0}")]
    Connect(String),
    #[error("비활성 상태 (REDIS_ENABLED=false)")]
    Disabled,
}

// === 실제 Redis 구현 (redis crate) ===

/// 실제 Redis 인스턴스와 연결된 L2 스토어.
///
/// - 캐시: Redis `GET`/`SETEX` (JSON 직렬화, TTL 1시간)
/// - Pub/Sub: Redis `PUBLISH`/`SUBSCRIBE` 채널 `meta:invalidation`
pub struct RealRedisStore {
    cache_conn: Arc<MultiplexedConnection>,
    client: Client,
}

impl RealRedisStore {
    /// Redis 인스턴스에 연결해 스토어 생성.
    ///
    /// - `cache_conn`: MultiplexedConnection (캐시용)
    /// - `client`: Pub/Sub용 일반 연결 생성 시 사용
    ///
    /// 연결 실패 시 `Err(Connect)` 반환.
    /// Phase 17: 3회 재시도 (2초 간격).
    pub async fn connect(config: &RedisConfig) -> Result<Self, RedisLikeError> {
        let client = Client::open(config.url.clone())
            .map_err(|e| RedisLikeError::Connect(e.to_string()))?;

        let mut last_err = String::new();
        for attempt in 1..=3 {
            match client.get_multiplexed_async_connection().await {
                Ok(cache_conn) => {
                    // 연결 확인
                    {
                        let mut conn = cache_conn.clone();
                        match redis::cmd("PING").query_async(&mut conn).await {
                            Ok(()) => {
                                println!("[Redis] 연결 성공 (시도 {attempt}/3)");
                                return Ok(Self {
                                    cache_conn: Arc::new(cache_conn),
                                    client,
                                });
                            }
                            Err(e) => {
                                last_err = format!("PING 실패: {e}");
                                println!("[Redis] 연결 실패 (시도 {attempt}/3): {last_err}");
                                if attempt < 3 {
                                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    last_err = e.to_string();
                    println!("[Redis] 연결 실패 (시도 {attempt}/3): {last_err}");
                    if attempt < 3 {
                        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    }
                }
            }
        }

        Err(RedisLikeError::Connect(last_err))
    }

    /// Pub/Sub 구독 루프를 실행.
    ///
    /// `handler`는 각 무효화 이벤트에 대해 호출된다.
    /// 채널 `meta:invalidation`에서 메시지를 수신하고,
    /// JSON 파싱 후 `handler`를 호출한다.
    /// 이 함수는 **block until error** (구독 루프 종료 시 반환).
    async fn subscribe_loop(
        client: &Client,
        mut handler: Box<dyn FnMut(InvalidationToken) + Send>,
    ) -> Result<(), RedisLikeError> {
        // Pub/Sub용 일반 연결 생성 (MultiplexedConnection은 into_pubsub 지원 안 함)
        let mut con = client
            .get_async_connection()
            .await
            .map_err(|e| RedisLikeError::Connect(e.to_string()))?;

        let mut pubsub = con.into_pubsub();
        pubsub
            .subscribe(INVALIDATION_CHANNEL)
            .await
            .map_err(|e| RedisLikeError::PubSub(format!("SUBSCRIBE 실패: {e}")))?;

        loop {
            let msg = pubsub
                .on_message()
                .next()
                .await
                .ok_or_else(|| RedisLikeError::PubSub("구독 루프 종료".to_string()))?;

            let payload = msg.get_payload_bytes();
            let text = match std::str::from_utf8(payload) {
                Ok(t) => t.to_string(),
                Err(_) => continue,
            };

            if let Ok(token) = serde_json::from_str::<InvalidationToken>(&text) {
                (handler)(token);
            }
        }
    }
}

#[async_trait::async_trait]
impl RedisLikeStore for RealRedisStore {
    async fn ping(&self) -> Result<(), RedisLikeError> {
        let mut conn = self.cache_conn.as_ref().clone();
        let _: String = redis::cmd("PING")
            .query_async(&mut conn)
            .await
            .map_err(|e| RedisLikeError::Connect(format!("PING: {e}")))?;
        Ok(())
    }

    async fn get_cache(&self, key: &str) -> Result<Option<CacheEntry>, RedisLikeError> {
        let mut conn = self.cache_conn.as_ref().clone();
        let val: Option<String> = conn
            .get(key)
            .await
            .map_err(|e| RedisLikeError::Cache(format!("GET {key}: {e}")))?;
        Ok(val.and_then(|v| serde_json::from_str::<CacheEntry>(&v).ok()))
    }

    async fn set_cache(&self, entry: &CacheEntry) -> Result<(), RedisLikeError> {
        let mut conn = self.cache_conn.as_ref().clone();
        let json = serde_json::to_string(entry)
            .map_err(|e| RedisLikeError::Cache(e.to_string()))?;
        let _: () = conn
            .set_ex(entry.key.as_str(), &json, 3600)
            .await
            .map_err(|e| RedisLikeError::Cache(format!("SET {}: {e}", entry.key)))?;
        Ok(())
    }

    async fn delete_cache(&self, key: &str) -> Result<(), RedisLikeError> {
        let mut conn = self.cache_conn.as_ref().clone();
        let _: () = conn
            .del(key)
            .await
            .map_err(|e| RedisLikeError::Cache(format!("DEL {key}: {e}")))?;
        Ok(())
    }

    async fn publish_invalidation(&self, token: &InvalidationToken) -> Result<(), RedisLikeError> {
        let json = serde_json::to_string(token)
            .map_err(|e| RedisLikeError::PubSub(e.to_string()))?;
        let mut conn = self.cache_conn.as_ref().clone();
        let published: i64 = redis::cmd("PUBLISH")
            .arg(INVALIDATION_CHANNEL)
            .arg(&json)
            .query_async(&mut conn)
            .await
            .map_err(|e| RedisLikeError::PubSub(format!("PUBLISH: {e}")))?;
        if published == 0 {
            eprintln!("[Redis] PUBLISH {INVALIDATION_CHANNEL} 수신자 0개");
        }
        Ok(())
    }

    async fn subscribe_invalidation(
        &self,
        mut handler: Box<dyn FnMut(InvalidationToken) + Send>,
    ) -> Result<(), RedisLikeError> {
        let client = self.client.clone();
        tokio::spawn(async move {
            if let Err(e) = RealRedisStore::subscribe_loop(&client, handler).await {
                eprintln!("[Redis] 구독 루프 종료: {e}");
            }
        });
        Ok(())
    }
}

// === 메모리 스텁 구현 (인스턴스 없는 환경) ===

/// 메모리 기반 L2 캐시 + Pub/Sub 스텁.
///
/// `broadcast` 채널로 in-process Pub/Sub을 시뮬레이션한다.
/// Phase 2의 Pub/Sub 발신/수신 로직을 실데이터 없이 검증한다.
pub struct InMemoryPubSub {
    cache: RwLock<HashMap<String, CacheEntry>>,
    pub_tx: broadcast::Sender<InvalidationToken>,
}

impl InMemoryPubSub {
    pub fn new() -> Self {
        let (pub_tx, _) = broadcast::channel(64);
        Self {
            cache: RwLock::new(HashMap::new()),
            pub_tx,
        }
    }
}

impl Default for InMemoryPubSub {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl RedisLikeStore for InMemoryPubSub {
    async fn ping(&self) -> Result<(), RedisLikeError> {
        Ok(())
    }

    async fn get_cache(&self, key: &str) -> Result<Option<CacheEntry>, RedisLikeError> {
        Ok(self.cache.read().await.get(key).cloned())
    }

    async fn set_cache(&self, entry: &CacheEntry) -> Result<(), RedisLikeError> {
        self.cache
            .write()
            .await
            .insert(entry.key.clone(), entry.clone());
        Ok(())
    }

    async fn delete_cache(&self, key: &str) -> Result<(), RedisLikeError> {
        self.cache.write().await.remove(key);
        Ok(())
    }

    async fn publish_invalidation(&self, token: &InvalidationToken) -> Result<(), RedisLikeError> {
        let _ = self.pub_tx.send(token.clone());
        Ok(())
    }

    async fn subscribe_invalidation(
        &self,
        mut handler: Box<dyn FnMut(InvalidationToken) + Send>,
    ) -> Result<(), RedisLikeError> {
        let mut sub = self.pub_tx.subscribe();
        tokio::spawn(async move {
            while let Ok(token) = sub.recv().await {
                handler(token);
            }
        });
        Ok(())
    }
}

/// 설정 기반 RedisLikeStore 팩토리.
///
/// - `enabled == true` → 실제 Redis 연결 (`RealRedisStore`). 연결 실패 시 `InMemoryPubSub`으로 폴백.
/// - `enabled == false` → `InMemoryPubSub` 스텁 (기본값)
pub async fn create_redis_store(
    config: &RedisConfig,
) -> Result<Arc<dyn RedisLikeStore>, RedisLikeError> {
    if config.enabled {
        match RealRedisStore::connect(config).await {
            Ok(store) => Ok(Arc::new(store)),
            Err(e) => {
                eprintln!("[Redis] 연결 실패: {e} → InMemoryPubSub 스텁으로 폴백");
                Ok(Arc::new(InMemoryPubSub::new()))
            }
        }
    } else {
        Ok(Arc::new(InMemoryPubSub::new()))
    }
}

/// 환경변수에서 RedisConfig 생성.
///
/// - `REDIS_ENABLED`: "true"/"false" (기본 "false")
/// - `REDIS_URL`: 연결 문자열 (기본 "redis://localhost:6379")
pub fn redis_config_from_env() -> RedisConfig {
    let enabled = std::env::var("REDIS_ENABLED")
        .unwrap_or_else(|_| "false".to_string())
        .eq_ignore_ascii_case("true");
    let url = std::env::var("REDIS_URL").unwrap_or_else(|_| {
        "redis://localhost:6379".to_string()
    });
    RedisConfig { enabled, url }
}
