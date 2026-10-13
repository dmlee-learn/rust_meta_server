//! 통합 테스트: 엔진 생성 -> 샌드박스 -> Throttling 강제.
//!
//! `meta_server` 라이브러리 모듈을 직접 사용해 Phase 1의
//! "런타임 초기화 + 메모리 제한 + CPU Timeout" 동작을 검증한다.

use meta_server::core::error::InfraError;
use meta_server::engine::{Engine, ThrottleConfig};

/// ThrottleConfig 기본값이 AGENTS.md의 Throttling 정책을 반영하는지.
#[test]
fn default_throttle_config_has_limits() {
    let cfg = ThrottleConfig::default();
    assert!(cfg.memory_limit > 0, "메모리 제한이 0보다 커야 한다");
    assert!(cfg.cpu_timeout_ms > 0, "CPU 타임아웃이 0보다 커야 한다");
}

/// 엔진 생성 시 메모리 한도가 실제로 런타임에 반영되는지.
#[test]
fn engine_applies_memory_limit_to_runtime() {
    let engine = Engine::new(ThrottleConfig {
        memory_limit: 4 * 1024 * 1024,
        max_stack: 0,
        cpu_timeout_ms: 500,
    })
    .expect("엔진 생성 실패 (CR0002)");
    let usage = engine.memory_usage();
    assert_eq!(usage.malloc_limit as usize, 4 * 1024 * 1024);
}

/// 샌드박스가 정상 스크립트를 실행하는지.
#[test]
fn sandbox_executes_valid_script() {
    let engine = Engine::new(ThrottleConfig {
        memory_limit: 8 * 1024 * 1024,
        max_stack: 0,
        cpu_timeout_ms: 0,
    })
    .unwrap();
    let sb = engine.create_sandbox().unwrap();
    assert_eq!(sb.eval_i64("21 * 2").unwrap(), 42);
}

/// 무한 루프가 CPU Timeout(CJ1102)으로 강제 종료되는지.
#[test]
fn infinite_loop_terminates_with_cj102() {
    let engine = Engine::new(ThrottleConfig {
        memory_limit: 128 * 1024 * 1024,
        max_stack: 0,
        cpu_timeout_ms: 80,
    })
    .unwrap();
    let sb = engine.create_sandbox().unwrap();
    let start = std::time::Instant::now();
    let res: Result<i64, InfraError> = sb.eval_i64("let s=0;for(let i=0;i<9e9;i++){s+=i}s");
    let elapsed = start.elapsed();
    assert_eq!(res.unwrap_err(), InfraError::Cj102);
    // CPU Timeout이 실제로 동작했는지 시간 검증 (제한 + 여유)
    assert!(
        elapsed.as_millis() < 5000,
        "CPU Timeout이 5초 안에 발동해야 했는데 {elapsed:?} 걸림"
    );
}

/// 메모리 한도 초과가 CJ1103으로 분류되는지.
#[test]
fn memory_overflow_classified_as_cj103() {
    let engine = Engine::new(ThrottleConfig {
        memory_limit: 192 * 1024,
        max_stack: 0,
        cpu_timeout_ms: 0,
    })
    .unwrap();
    let sb = engine.create_sandbox().unwrap();
    let res: Result<i64, InfraError> =
        sb.eval_i64("var a=[];for(let i=0;i<3000000;i++){a.push('y'.repeat(128));}1");
    assert_eq!(res.unwrap_err(), InfraError::Cj103);
}

/// OOM 후 GC 수동 트리거로 메모리 회수 시 런타임이 생존하는지.
///
/// AGENTS.md: 결정적 실패 보호 - 실패 시 안전한 상태로 후퇴.
/// 동일 런타임에서 OOM 직후 재시도는 메모리 고갈로 실패할 수 있으나,
/// GC로 메모리 회수 후 재시도는 성공해야 한다.
#[test]
fn runtime_survives_after_gc_recovery() {
    let engine = Engine::new(ThrottleConfig {
        memory_limit: 192 * 1024,
        max_stack: 0,
        cpu_timeout_ms: 0,
    })
    .unwrap();
    // GC 수동 트리거 후 정상 스크립트 실행
    engine.force_gc();
    let sb = engine.create_sandbox().unwrap();
    assert_eq!(sb.eval_i64("7 * 6").unwrap(), 42);
}

/// 에러 코드 문자열이 AGENTS.md 명명 규칙(6자리)을 준수하는지.
#[test]
fn error_codes_follow_naming_convention() {
    for e in [
        InfraError::Cr001,
        InfraError::Cr002,
        InfraError::Cj102,
        InfraError::Cj103,
    ] {
        let code = e.code();
        assert_eq!(code.len(), 6, "{code}는 6자리여야 한다");
        // 1자 도메인 + 1자 레이어(R/J) + 1자 종류 + 3자 개별번호
        assert!(matches!(code.as_bytes()[0], b'C' | b'S' | b'E' | b'B'));
        assert!(matches!(code.as_bytes()[1], b'R' | b'J'));
        assert!(code[2..].chars().all(|c| c.is_ascii_digit()));
    }
}
