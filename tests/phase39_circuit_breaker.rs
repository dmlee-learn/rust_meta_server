//! Phase 39 테스트: Token Bucket + Circuit Breaker.

use std::time::Duration;
use meta_server::rate_limit::{CircuitBreaker, CircuitState, TokenBucket};

// =========================================================
// Token Bucket
// =========================================================

/// 새 버킷은 capacity만큼 즉시 소비 가능.
#[tokio::test]
async fn token_bucket_initial_capacity() {
    let mut b = TokenBucket::new(3, 1.0);
    assert!(b.take());
    assert!(b.take());
    assert!(b.take());
    assert!(!b.take(), "capacity 초과 시 거부");
}

/// 토큰 재충전이 이루어진다.
#[tokio::test]
async fn token_bucket_refill() {
    let mut b = TokenBucket::new(1, 100.0); // 100/sec
    assert!(b.take());
    assert!(!b.take());
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert!(b.take(), "재충전 후 다시 소비 가능");
}

/// take_many: 대량 소비.
#[tokio::test]
async fn token_bucket_take_many() {
    let mut b = TokenBucket::new(5, 0.1);
    assert!(b.take_many(5));
    assert!(!b.take_many(1), "전부 소진");
}

// =========================================================
// Circuit Breaker
// =========================================================

/// 처음에는 Closed.
#[test]
fn circuit_initially_closed() {
    let mut cb = CircuitBreaker::new(3, Duration::from_secs(60));
    assert_eq!(cb.state(), CircuitState::Closed);
    assert!(cb.allow_request());
}

/// 연속 실패가 threshold에 도달하면 Open.
#[test]
fn circuit_opens_after_threshold() {
    let mut cb = CircuitBreaker::new(3, Duration::from_secs(60));
    cb.record_failure();
    cb.record_failure();
    assert_eq!(cb.state(), CircuitState::Closed);
    cb.record_failure(); // 3번째
    assert_eq!(cb.state(), CircuitState::Open);
    assert!(!cb.allow_request(), "Open 상태에서는 요청 거부");
}

/// 성공 시 Closed로 복귀.
#[test]
fn circuit_closes_on_success() {
    let mut cb = CircuitBreaker::new(3, Duration::from_secs(60));
    cb.record_failure();
    cb.record_failure();
    cb.record_success();
    assert_eq!(cb.state(), CircuitState::Closed);
    assert!(cb.allow_request());
}

/// Open 상태 후 cooldown을 지나면 HalfOpen으로 자동 전환.
#[test]
fn circuit_half_open_after_cooldown() {
    let mut cb = CircuitBreaker::new(2, Duration::from_millis(30));
    cb.record_failure();
    cb.record_failure();
    assert_eq!(cb.state(), CircuitState::Open);
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(cb.state(), CircuitState::HalfOpen);
    assert!(cb.allow_request(), "HalfOpen에서 탐색 요청 허용");
}

/// HalfOpen에서 실패하면 다시 Open.
#[test]
fn circuit_reopens_on_half_open_failure() {
    let mut cb = CircuitBreaker::new(2, Duration::from_millis(20));
    cb.record_failure();
    cb.record_failure();
    std::thread::sleep(Duration::from_millis(40));
    assert_eq!(cb.state(), CircuitState::HalfOpen);
    let _ = cb.allow_request();
    cb.record_failure();
    assert_eq!(cb.state(), CircuitState::Open);
}

/// open_remaining_ms가 Open 상태에서 positive 반환.
#[test]
fn circuit_open_remaining_ms() {
    let mut cb = CircuitBreaker::new(2, Duration::from_secs(60));
    assert_eq!(cb.open_remaining_ms(), 0, "Closed 상태는 0");
    cb.record_failure();
    cb.record_failure();
    assert!(cb.open_remaining_ms() > 0, "Open 상태에서 남은 시간 > 0");
}
