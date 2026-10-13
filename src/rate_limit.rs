//! Rate Limiting 미들웨어 (Phase 15, Phase 18 확장).
//!
//! IP 기반 sliding window rate limiting.
//! 분당 `RATE_LIMIT_PER_MIN` 회 초과 시 429 Too Many Requests.
//! Phase 18: per-domain rate limit.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use tokio::sync::RwLock;

use crate::http::AppState;

// =============================================================
// Phase 39: Token Bucket + Circuit Breaker
// =============================================================

/// Phase 39: 토큰 버킷 (sliding window 대안).
///
/// `capacity` 버킷에 `refill_rate`/초 비율로 토큰이 다시 차오른다.
/// 요청은 1개 토큰을 소진하며, 토큰이 없으면 대기(또는 거부).
/// burst 허용을 원하면 capacity를 높게, 평상시 속도를 유지하려면
/// refill_rate를 낮게 설정한다.
#[derive(Debug, Clone)]
pub struct TokenBucket {
    capacity: f64,
    refill_rate: f64, // 토큰/초
    tokens: f64,
    last_refill: Instant,
}

impl TokenBucket {
    pub fn new(capacity: u32, refill_rate_per_sec: f64) -> Self {
        Self {
            capacity: capacity as f64,
            refill_rate: refill_rate_per_sec,
            tokens: capacity as f64,
            last_refill: Instant::now(),
        }
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.refill_rate).min(self.capacity);
        self.last_refill = now;
    }

    /// 1개 토큰을 소진할 수 있으면 `true`.
    pub fn take(&mut self) -> bool {
        self.refill();
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    /// 필요한 토큰 수를 소진할 수 있으면 `true`.
    pub fn take_many(&mut self, n: u32) -> bool {
        self.refill();
        let need = n as f64;
        if self.tokens >= need {
            self.tokens -= need;
            true
        } else {
            false
        }
    }

    pub fn available(&mut self) -> f64 {
        self.refill();
        self.tokens
    }
}

/// Phase 39: 서킷 브레이커 상태.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed,    // 정상: 요청 통과
    Open,      // 장애: 요청 즉시 거부 (콜다운 대기)
    HalfOpen,  // 콜다운 종료 후 탐색: 일부 요청만 통과
}

/// Phase 39: 서킷 브레이커.
///
/// 연속 실패가 `failure_threshold`에 도달하면 **Open**으로 전환되어
/// `cooldown` 동안 요청을 거부한다. 콜다운이 끝나면 **HalfOpen**으로
/// 진입해 소수의 탐색 요청을 허용하며, 성공하면 **Closed**로 복귀하고
/// 실패하면 다시 **Open**으로 전환된다.
#[derive(Debug)]
pub struct CircuitBreaker {
    failure_threshold: u32,
    cooldown: Duration,
    state: CircuitState,
    consecutive_failures: u32,
    opened_at: Option<Instant>,
    half_open_probe_allowed: u32,
}

impl CircuitBreaker {
    pub fn new(failure_threshold: u32, cooldown: Duration) -> Self {
        Self {
            failure_threshold,
            cooldown,
            state: CircuitState::Closed,
            consecutive_failures: 0,
            opened_at: None,
            half_open_probe_allowed: 0,
        }
    }

    pub fn state(&self) -> CircuitState {
        // Open 상태에서 cooldown이 지나면 HalfOpen으로 자동 전환(읽기 시).
        if self.state == CircuitState::Open {
            if let Some(at) = self.opened_at {
                if at.elapsed() >= self.cooldown {
                    return CircuitState::HalfOpen;
                }
            }
        }
        self.state
    }

    /// 요청을 허용할지 결정. 허용되면 상태 업데이트.
    pub fn allow_request(&mut self) -> bool {
        match self.state() {
            CircuitState::Closed => {
                true
            }
            CircuitState::Open => false,
            CircuitState::HalfOpen => {
                // HalfOpen에서는 제한된 탐색 요청만 허용.
                if self.half_open_probe_allowed == 0 {
                    // cooldown이 끝난 지점: 탐색 기회 갱신.
                    self.half_open_probe_allowed = 3; // 최대 3개 탐색
                }
                if self.half_open_probe_allowed > 0 {
                    self.half_open_probe_allowed -= 1;
                    true
                } else {
                    false
                }
            }
        }
    }

    /// 요청 성공 기록.
    pub fn record_success(&mut self) {
        self.consecutive_failures = 0;
        self.state = CircuitState::Closed;
        self.opened_at = None;
    }

    /// 요청 실패 기록.
    pub fn record_failure(&mut self) {
        match self.state() {
            CircuitState::HalfOpen => {
                // 탐색 실패: 다시 Open.
                self.state = CircuitState::Open;
                self.opened_at = Some(Instant::now());
                self.half_open_probe_allowed = 0;
            }
            _ => {
                self.consecutive_failures += 1;
                if self.consecutive_failures >= self.failure_threshold && self.state() == CircuitState::Closed {
                    self.state = CircuitState::Open;
                    self.opened_at = Some(Instant::now());
                }
            }
        }
    }

    /// 현재 상태가 Open이고 cooldown이 남은 ms (HalfOpen/Closed는 0).
    pub fn open_remaining_ms(&self) -> u64 {
        if self.state == CircuitState::Open {
            if let Some(at) = self.opened_at {
                let elapsed = at.elapsed();
                if elapsed < self.cooldown {
                    return (self.cooldown - elapsed).as_millis() as u64;
                }
            }
        }
        0
    }
}

/// Rate Limiting 설정.
#[derive(Debug, Clone, Default)]
pub struct RateLimitConfig {
    /// 분당 최대 요청 수 (global).
    pub per_minute: u32,
    /// domain별 분당 최대 요청 수 (Phase 18).
    pub per_domain: HashMap<String, u32>,
    /// Phase 19: user별 분당 최대 요청 수.
    pub user_per_minute: u32,
}

impl RateLimitConfig {
    /// 환경변수에서 설정 로드.
    pub fn from_env() -> Self {
        let per_minute = std::env::var("RATE_LIMIT_PER_MIN")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(60);

        let user_per_minute = std::env::var("RATE_LIMIT_USER_PER_MIN")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(60);

        // Phase 18: per-domain rate limit
        let mut per_domain = HashMap::new();
        for domain in ["board", "erp", "blog", "shop"] {
            let env_key = format!("RATE_LIMIT_{}_PER_MIN", domain.to_uppercase());
            if let Ok(v) = std::env::var(&env_key) {
                if let Ok(n) = v.parse::<u32>() {
                    per_domain.insert(domain.to_string(), n);
                }
            }
        }

        Self {
            per_minute,
            per_domain,
            user_per_minute,
        }
    }
}

/// IP별 요청 카운트.
#[derive(Debug, Default)]
pub struct RateLimiter {
    /// global: ip -> (count, start)
    counts: RwLock<HashMap<String, (u64, Instant)>>,
    /// Phase 18: domain별: (ip, domain) -> (count, start)
    domain_counts: RwLock<HashMap<(String, String), (u64, Instant)>>,
    /// Phase 19: user별: user -> (count, start)
    user_counts: RwLock<HashMap<String, (u64, Instant)>>,
    config: RateLimitConfig,
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            counts: RwLock::new(HashMap::new()),
            domain_counts: RwLock::new(HashMap::new()),
            user_counts: RwLock::new(HashMap::new()),
            config,
        }
    }

    /// 요청 체크: 제한 초과 시 `Err(retry_after_ms)` 반환.
    pub async fn check(&self, ip: &str) -> Result<(), u64> {
        // global limit check
        let mut counts = self.counts.write().await;
        let now = Instant::now();

        if let Some((count, start)) = counts.get_mut(ip) {
            if now.duration_since(*start) >= Duration::from_secs(60) {
                *count = 0;
                *start = now;
            }

            if *count >= self.config.per_minute as u64 {
                let elapsed = now.duration_since(*start).as_millis() as u64;
                let remaining = 60_000 - elapsed;
                return Err(remaining);
            }

            *count += 1;
        } else {
            counts.insert(ip.to_string(), (1, now));
        }

        Ok(())
    }

    /// Phase 18: domain별 요청 체크.
    pub async fn check_domain(&self, ip: &str, domain: &str) -> Result<(), u64> {
        let limit = self
            .config
            .per_domain
            .get(domain)
            .copied()
            .unwrap_or(self.config.per_minute);

        let mut domain_counts = self.domain_counts.write().await;
        let now = Instant::now();
        let key = (ip.to_string(), domain.to_string());

        if let Some((count, start)) = domain_counts.get_mut(&key) {
            if now.duration_since(*start) >= Duration::from_secs(60) {
                *count = 0;
                *start = now;
            }

            if *count >= limit as u64 {
                let elapsed = now.duration_since(*start).as_millis() as u64;
                let remaining = 60_000 - elapsed;
                return Err(remaining);
            }

            *count += 1;
            Ok(())
        } else {
            domain_counts.insert(key, (1, now));
            Ok(())
        }
    }
    /// Phase 19: user별 요청 체크.
    pub async fn check_user(&self, user: &str) -> Result<(), u64> {
        let limit = self.config.user_per_minute;
        let mut user_counts = self.user_counts.write().await;
        let now = Instant::now();

        if let Some((count, start)) = user_counts.get_mut(user) {
            if now.duration_since(*start) >= Duration::from_secs(60) {
                *count = 0;
                *start = now;
            }

            if *count >= limit as u64 {
                let elapsed = now.duration_since(*start).as_millis() as u64;
                let remaining = 60_000 - elapsed;
                return Err(remaining);
            }

            *count += 1;
            Ok(())
        } else {
            user_counts.insert(user.to_string(), (1, now));
            Ok(())
        }
    }
}

/// Rate Limiting 미들웨어.
/// Phase 18: per-domain rate limit 적용.
/// Phase 19: user-based rate limit 적용.
pub async fn rate_limit_middleware(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    // 인증 불필요 경로 제외
    let path = request.uri().path().to_string();
    if path == "/health" || path == "/metrics" || path.starts_with("/api/engine") || path.starts_with("/admin") || path.starts_with("/api/admin") {
        return next.run(request).await;
    }

    // Phase 39: 서킷 브레이커 — Open이면 즉시 거부.
    {
        let mut cb = state.circuit_breaker.write().await;
        if !cb.allow_request() {
            let remain = cb.open_remaining_ms();
            let retry_after_sec = (remain + 999) / 1000;
            return Response::builder()
                .status(StatusCode::TOO_MANY_REQUESTS)
                .header("retry-after", retry_after_sec.to_string())
                .body(
                    axum::body::Body::from(
                        serde_json::json!({
                            "code": "CR4004",
                            "message": "Service temporarily unavailable (circuit breaker open)",
                            "retry_after_ms": remain,
                        })
                        .to_string(),
                    )
                )
                .unwrap();
        }
    }

    // Phase 22: HTTP 요청 기록
    let domain = extract_domain_from_path(&path);
    let endpoint = domain.as_deref().unwrap_or("other");
    state.metrics.record_http_request(endpoint);

    // 클라이언트 IP 추출
    let ip = extract_client_ip(&request);

    // Phase 18: domain 추출
    let domain_opt = extract_domain_from_path(&path);

    // Phase 19: user 추출 (JWT sub)
    let user = extract_user_from_request(&request);

    // global limit check
    let response = match state.rate_limiter.check(&ip).await {
        Ok(()) => {
            // Phase 18: domain limit check
            if let Some(ref dom) = domain_opt {
                match state.rate_limiter.check_domain(&ip, dom).await {
                    Ok(()) => {
                        // Phase 19: user limit check
                        if let Some(ref usr) = user {
                            match state.rate_limiter.check_user(usr).await {
                                Ok(()) => next.run(request).await,
                                Err(retry_after_ms) => {
                                    let retry_after_sec = (retry_after_ms + 999) / 1000;
                                    return Response::builder()
                                        .status(StatusCode::TOO_MANY_REQUESTS)
                                        .header("retry-after", retry_after_sec.to_string())
                                        .body(
                                            axum::body::Body::from(
                                                serde_json::json!({
                                                    "code": "CR4003",
                                                    "message": "User rate limit exceeded",
                                                    "retry_after_ms": retry_after_ms,
                                                })
                                                .to_string(),
                                            )
                                        )
                                        .unwrap();
                                }
                            }
                        } else {
                            next.run(request).await
                        }
                    }
                    Err(retry_after_ms) => {
                        let retry_after_sec = (retry_after_ms + 999) / 1000;
                        return Response::builder()
                            .status(StatusCode::TOO_MANY_REQUESTS)
                            .header("retry-after", retry_after_sec.to_string())
                            .body(
                                axum::body::Body::from(
                                    serde_json::json!({
                                        "code": "CR4002",
                                        "message": format!("Domain rate limit exceeded ({dom})"),
                                        "retry_after_ms": retry_after_ms,
                                    })
                                    .to_string(),
                                )
                            )
                            .unwrap();
                    }
                }
            } else {
                next.run(request).await
            }
        }
        Err(retry_after_ms) => {
            let retry_after_sec = (retry_after_ms + 999) / 1000; // ceil
            Response::builder()
                .status(StatusCode::TOO_MANY_REQUESTS)
                .header("retry-after", retry_after_sec.to_string())
                .body(
                    axum::body::Body::from(
                        serde_json::json!({
                            "code": "CR4001",
                            "message": "Rate limit exceeded",
                            "retry_after_ms": retry_after_ms,
                        })
                        .to_string(),
                    )
                )
                .unwrap()
        }
    };

    // Phase 39: 서킷 브레이커 성공/실패 기록.
    // 5xx는 하류(DB/비즈니스) 실패로 간주, 그 외는 성공.
    let cb_outcome = match response.status() {
        s if s.is_server_error() => false,
        _ => true,
    };
    {
        let mut cb = state.circuit_breaker.write().await;
        if cb_outcome {
            cb.record_success();
        } else {
            cb.record_failure();
        }
    }

    response
}

/// Phase 18: path에서 domain 추출.
fn extract_domain_from_path(path: &str) -> Option<String> {
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if parts.len() >= 2 && parts[0] == "api" {
        Some(parts[1].to_string())
    } else {
        None
    }
}

/// Phase 19: JWT sub에서 user 추출.
fn extract_user_from_request(request: &Request) -> Option<String> {
    // Authorization: Bearer <token>
    if let Some(auth) = request.headers().get("authorization") {
        if let Ok(s) = auth.to_str() {
            if let Some(token) = s.strip_prefix("Bearer ") {
                // 간단히 token의 첫 부분으로 user 식별 (실제로는 JWT decode 필요)
                // Phase 19: JWT decode를 위해 claims를 추출
                if let Ok(claims) = crate::auth::decode_token_claims(token) {
                    return Some(claims.sub);
                }
            }
        }
    }
    None
}

/// 클라이언트 IP 추출.
fn extract_client_ip(request: &Request) -> String {
    // X-Forwarded-For 헤더 우선
    if let Some(xff) = request.headers().get("x-forwarded-for") {
        if let Ok(s) = xff.to_str() {
            return s.split(',').next().unwrap_or("unknown").trim().to_string();
        }
    }

    // 소켓 어드레스
    if let Some(xri) = request.headers().get("x-real-ip") {
        if let Ok(s) = xri.to_str() {
            return s.to_string();
        }
    }

    "unknown".to_string()
}
