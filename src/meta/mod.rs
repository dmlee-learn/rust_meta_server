//! PostgreSQL 기반 기준 메타데이터 모델 + Redis L2 캐시.
//!
//! L3(PostgreSQL) = `store`, L2(Redis) = `redis`.
//! AGENTS.md 철학: 단순 상태 값 변경은 **메타데이터(선언적)**로,
//! 분기/알고리즘은 **스크립트(명령형)**로 분리.

pub mod migrate;
pub mod redis;
pub mod store;

pub use migrate::run_migrations;
pub use redis::{
    CacheEntry, InMemoryPubSub, InvalidationToken, RealRedisStore, RedisConfig, RedisLikeError,
    RedisLikeStore, create_redis_store, redis_config_from_env,
};
pub use store::{
    DataStore, DataStoreError, MemoryStore, MetaModule, MetaStoreConfig, MetaTemplate, PostgresStore,
    TemplateTree, create_data_store, seed_templates_from_files,
};
