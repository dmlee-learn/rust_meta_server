//! Prometheus Metrics 레이어 (Phase 14, Phase 17 확장).
//!
//! Prometheus text format을 직접 생성하는 lightweight metrics.
//! `prometheus-client` crate의 복잡한 API를 대신해 간단한 동시성 카운터 사용.
//! Phase 17: HTTP request duration histogram 추가.

use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;

/// 서버 전역 메트릭스.
#[derive(Debug)]
pub struct Metrics {
    /// HTTP 요청 카운터 (엔드포인트별).
    pub http_requests: HttpRequests,
    /// 캐시 적중 카운터.
    pub cache_hits: CacheHits,
    /// 캐시 미스 카운터.
    pub cache_misses: CacheMisses,
    /// 훅 실행 카운터.
    pub hook_executions: AtomicU64,
    /// 훅 에러 카운터.
    pub hook_errors: AtomicU64,
    /// 활성 WebSocket 세션 수.
    pub active_sessions: AtomicI64,
    /// 엔진 메모리 사용량 (bytes).
    pub engine_memory: AtomicI64,
    /// HTTP 요청 지연시간 histogram (Phase 17).
    pub http_duration: RwLock<HttpDuration>,
    /// Phase 35: 총 요청 수 (SLO availability 계산용).
    pub total_requests: AtomicU64,
    /// Phase 35: 총 에러 요청 수 (5xx).
    pub total_errors: AtomicU64,
}

impl Default for Metrics {
    fn default() -> Self {
        Self {
            http_requests: HttpRequests::default(),
            cache_hits: CacheHits::default(),
            cache_misses: CacheMisses::default(),
            hook_executions: AtomicU64::new(0),
            hook_errors: AtomicU64::new(0),
            active_sessions: AtomicI64::new(0),
            engine_memory: AtomicI64::new(0),
            http_duration: RwLock::new(HttpDuration::default()),
            total_requests: AtomicU64::new(0),
            total_errors: AtomicU64::new(0),
        }
    }
}

/// 엔드포인트별 HTTP 요청 카운터.
#[derive(Debug, Default)]
pub struct HttpRequests {
    pub board: AtomicU64,
    pub erp: AtomicU64,
    pub blog: AtomicU64,
    pub shop: AtomicU64,
    pub health: AtomicU64,
    pub other: AtomicU64,
}

/// 캐시 적중 카운터.
#[derive(Debug, Default)]
pub struct CacheHits {
    pub l1: AtomicU64,
    pub l2: AtomicU64,
    pub l3: AtomicU64,
}

/// 캐시 미스 카운터.
#[derive(Debug, Default)]
pub struct CacheMisses {
    pub l1: AtomicU64,
    pub l2: AtomicU64,
    pub l3: AtomicU64,
}

/// HTTP 요청 지연시간 histogram (Phase 17).
///
/// bucket: 1, 5, 10, 50, 100, 500, 1000ms
/// per-domain label: board, erp, blog, shop, other
#[derive(Debug, Default)]
pub struct HttpDuration {
    /// domain -> (sum_ms, count, [bucket0, bucket1, ..., bucket6])
    entries: HashMap<String, (f64, u64, [u64; 7])>,
}

impl Metrics {
    pub fn new() -> Self {
        Self::default()
    }

    /// 엔드포인트별 HTTP 요청 기록.
    pub fn record_http_request(&self, endpoint: &str) {
        match endpoint {
            "board" => self.http_requests.board.fetch_add(1, Ordering::SeqCst),
            "erp" => self.http_requests.erp.fetch_add(1, Ordering::SeqCst),
            "blog" => self.http_requests.blog.fetch_add(1, Ordering::SeqCst),
            "shop" => self.http_requests.shop.fetch_add(1, Ordering::SeqCst),
            "health" => self.http_requests.health.fetch_add(1, Ordering::SeqCst),
            _ => self.http_requests.other.fetch_add(1, Ordering::SeqCst),
        };
    }

    /// 캐시 적중 기록.
    pub fn record_cache_hit(&self, layer: &str) {
        match layer {
            "l1" => self.cache_hits.l1.fetch_add(1, Ordering::SeqCst),
            "l2" => self.cache_hits.l2.fetch_add(1, Ordering::SeqCst),
            _ => self.cache_hits.l3.fetch_add(1, Ordering::SeqCst),
        };
    }

    /// 캐시 미스 기록.
    pub fn record_cache_miss(&self, layer: &str) {
        match layer {
            "l1" => self.cache_misses.l1.fetch_add(1, Ordering::SeqCst),
            "l2" => self.cache_misses.l2.fetch_add(1, Ordering::SeqCst),
            _ => self.cache_misses.l3.fetch_add(1, Ordering::SeqCst),
        };
    }

    /// 훅 실행 기록.
    pub fn record_hook_execution(&self) {
        self.hook_executions.fetch_add(1, Ordering::SeqCst);
    }

    /// 훅 에러 기록.
    pub fn record_hook_error(&self) {
        self.hook_errors.fetch_add(1, Ordering::SeqCst);
    }

    /// HTTP 요청 지연시간 기록 (Phase 17).
    pub async fn record_http_duration(&self, domain: &str, elapsed_ms: f64) {
        let mut duration = self.http_duration.write().await;
        let entry = duration.entries.entry(domain.to_string()).or_default();
        entry.0 += elapsed_ms;
        entry.1 += 1;

        // bucket: [1, 5, 10, 50, 100, 500, 1000]ms
        let buckets = [1.0, 5.0, 10.0, 50.0, 100.0, 500.0, 1000.0];
        for (i, threshold) in buckets.iter().enumerate() {
            if elapsed_ms <= *threshold {
                entry.2[i] += 1;
                break;
            }
        }
    }

    /// Prometheus text format으로 인코딩 (Phase 17: async).
    pub async fn encode(&self) -> String {
        let mut s = String::new();

        s.push_str("# HELP meta_server_http_requests_total HTTP requests by endpoint\n");
        s.push_str("# TYPE meta_server_http_requests_total counter\n");
        s.push_str(&format!(
            "meta_server_http_requests_total{{endpoint=\"board\"}} {}\n",
            self.http_requests.board.load(Ordering::SeqCst)
        ));
        s.push_str(&format!(
            "meta_server_http_requests_total{{endpoint=\"erp\"}} {}\n",
            self.http_requests.erp.load(Ordering::SeqCst)
        ));
        s.push_str(&format!(
            "meta_server_http_requests_total{{endpoint=\"blog\"}} {}\n",
            self.http_requests.blog.load(Ordering::SeqCst)
        ));
        s.push_str(&format!(
            "meta_server_http_requests_total{{endpoint=\"shop\"}} {}\n",
            self.http_requests.shop.load(Ordering::SeqCst)
        ));
        s.push_str(&format!(
            "meta_server_http_requests_total{{endpoint=\"health\"}} {}\n",
            self.http_requests.health.load(Ordering::SeqCst)
        ));
        s.push_str(&format!(
            "meta_server_http_requests_total{{endpoint=\"other\"}} {}\n",
            self.http_requests.other.load(Ordering::SeqCst)
        ));

        s.push_str("# HELP meta_server_cache_hits_total Cache hits by layer\n");
        s.push_str("# TYPE meta_server_cache_hits_total counter\n");
        s.push_str(&format!(
            "meta_server_cache_hits_total{{layer=\"l1\"}} {}\n",
            self.cache_hits.l1.load(Ordering::SeqCst)
        ));
        s.push_str(&format!(
            "meta_server_cache_hits_total{{layer=\"l2\"}} {}\n",
            self.cache_hits.l2.load(Ordering::SeqCst)
        ));
        s.push_str(&format!(
            "meta_server_cache_hits_total{{layer=\"l3\"}} {}\n",
            self.cache_hits.l3.load(Ordering::SeqCst)
        ));

        s.push_str("# HELP meta_server_cache_misses_total Cache misses by layer\n");
        s.push_str("# TYPE meta_server_cache_misses_total counter\n");
        s.push_str(&format!(
            "meta_server_cache_misses_total{{layer=\"l1\"}} {}\n",
            self.cache_misses.l1.load(Ordering::SeqCst)
        ));
        s.push_str(&format!(
            "meta_server_cache_misses_total{{layer=\"l2\"}} {}\n",
            self.cache_misses.l2.load(Ordering::SeqCst)
        ));
        s.push_str(&format!(
            "meta_server_cache_misses_total{{layer=\"l3\"}} {}\n",
            self.cache_misses.l3.load(Ordering::SeqCst)
        ));

        s.push_str("# HELP meta_server_hook_executions_total Total hook executions\n");
        s.push_str("# TYPE meta_server_hook_executions_total counter\n");
        s.push_str(&format!(
            "meta_server_hook_executions_total {}\n",
            self.hook_executions.load(Ordering::SeqCst)
        ));

        s.push_str("# HELP meta_server_hook_errors_total Total hook errors\n");
        s.push_str("# TYPE meta_server_hook_errors_total counter\n");
        s.push_str(&format!(
            "meta_server_hook_errors_total {}\n",
            self.hook_errors.load(Ordering::SeqCst)
        ));

        s.push_str("# HELP meta_server_active_sessions Active WebSocket sessions\n");
        s.push_str("# TYPE meta_server_active_sessions gauge\n");
        s.push_str(&format!(
            "meta_server_active_sessions {}\n",
            self.active_sessions.load(Ordering::SeqCst)
        ));

        s.push_str("# HELP meta_server_engine_memory_bytes rquickjs engine memory usage\n");
        s.push_str("# TYPE meta_server_engine_memory_bytes gauge\n");
        s.push_str(&format!(
            "meta_server_engine_memory_bytes {}\n",
            self.engine_memory.load(Ordering::SeqCst)
        ));

        // Phase 17: HTTP duration histogram
        s.push_str("# HELP meta_server_http_request_duration_ms HTTP request duration in ms\n");
        s.push_str("# TYPE meta_server_http_request_duration_ms histogram\n");
        let duration = self.http_duration.read().await;
        for (domain, (sum, count, buckets)) in duration.entries.iter() {
            let bucket_thresholds = [1.0, 5.0, 10.0, 50.0, 100.0, 500.0, 1000.0];
            let mut cumulative: u64 = 0;
            for (i, threshold) in bucket_thresholds.iter().enumerate() {
                cumulative = buckets[i];
                s.push_str(&format!(
                    "meta_server_http_request_duration_ms_bucket{{domain=\"{}\",le=\"{}\"}} {}\n",
                    domain, threshold, cumulative
                ));
            }
            s.push_str(&format!(
                "meta_server_http_request_duration_ms_bucket{{domain=\"{}\",le=\"+Inf\"}} {}\n",
                domain, count
            ));
            s.push_str(&format!(
                "meta_server_http_request_duration_ms_sum{{domain=\"{}\"}} {}\n",
                domain, sum
            ));
            s.push_str(&format!(
                "meta_server_http_request_duration_ms_count{{domain=\"{}\"}} {}\n",
                domain, count
            ));
        }

        // Phase 35: SLO Availability + Error Budget
        let total = self.total_requests.load(Ordering::SeqCst) as f64;
        let errors = self.total_errors.load(Ordering::SeqCst) as f64;
        let availability = if total > 0.0 {
            (total - errors) / total
        } else {
            1.0
        };
        let slo_target = 0.999;
        let error_budget_ratio = (slo_target - availability).max(0.0);

        s.push_str("# HELP meta_server_slo_availability_ratio Current availability ratio\n");
        s.push_str("# TYPE meta_server_slo_availability_ratio gauge\n");
        s.push_str(&format!(
            "meta_server_slo_availability_ratio {}\n",
            availability
        ));
        s.push_str("# HELP meta_server_slo_error_budget_ratio Consumed error budget ratio (0=none, 1=exhausted)\n");
        s.push_str("# TYPE meta_server_slo_error_budget_ratio gauge\n");
        s.push_str(&format!(
            "meta_server_slo_error_budget_ratio {}\n",
            error_budget_ratio
        ));
        s.push_str("# HELP meta_server_total_requests_total Total HTTP requests\n");
        s.push_str("# TYPE meta_server_total_requests_total counter\n");
        s.push_str(&format!(
            "meta_server_total_requests_total {}\n",
            self.total_requests.load(Ordering::SeqCst)
        ));
        s.push_str("# HELP meta_server_total_errors_total Total 5xx error requests\n");
        s.push_str("# TYPE meta_server_total_errors_total counter\n");
        s.push_str(&format!(
            "meta_server_total_errors_total {}\n",
            self.total_errors.load(Ordering::SeqCst)
        ));

        s
    }
}
