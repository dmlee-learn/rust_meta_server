//! 스크립트 실행 옵저버: 이벤트 큐 + ExecutionReport.
//!
//! Q5-3: 이벤트 기반 옵저버 + 요청당 실행 리포트.
//! Q6-2: 독립 모듈 (엔진은 ScriptObserver 트레잇으로만 참조).
//! Q7-2: 제한 강제 종료 + 지속 시간 임계값 경고.
//!
//! AGENTS.md: 인프라 제어권은 Rust, 비즈니스 정책은 스크립트.
//! 옵저버는 "관찰 + 기록 + 경보" 역할 (제한 강제와 별개).

use std::sync::Arc;

use tokio::sync::mpsc;

use crate::core::error::InfraError;

/// 실행 리포트 (Q5-2): 요청당 구조화된 실행 통계.
#[derive(Debug, Clone)]
pub struct ExecutionReport {
    /// 모듈 ID (로딩된 스크립트 식별).
    pub module_id: String,
    /// 스크립트 버전 (낙관적 버전 바인딩).
    pub version: u64,
    /// 시작 시각 (std::time::Instant).
    pub start: std::time::Instant,
    /// 종료 시각.
    pub end: std::time::Instant,
    /// 지속 시간.
    pub elapsed: std::time::Duration,
    /// 제한(CPU/메모리) 초과 여부.
    pub exceeded: bool,
    /// 초과 시 에러 코드 (CJ1102/CJ1103).
    pub error_code: Option<String>,
    /// 메모리 피크 (바이트, 실행 직후 측정).
    pub memory_peak: usize,
}

impl ExecutionReport {
    /// 지속 시간(millis) > 임계값이면 true (Q7-2 경고 조건).
    pub fn is_slow(&self, threshold_ms: u64) -> bool {
        self.elapsed.as_millis() as u64 >= threshold_ms
    }

    /// 리포트를 JSON 값으로 직렬화 (모니터링/로깅용).
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "module_id": self.module_id,
            "version": self.version,
            "elapsed_ms": self.elapsed.as_millis() as u64,
            "exceeded": self.exceeded,
            "error_code": self.error_code,
            "memory_peak": self.memory_peak,
        })
    }
}

/// 스크립트 실행 이벤트 (Q5-1).
#[derive(Debug, Clone)]
pub enum EvalEvent {
    /// 실행 시작.
    Start {
        module_id: String,
        version: u64,
    },
    /// 실행 종료 (성공).
    End {
        module_id: String,
        report: ExecutionReport,
    },
    /// 실행 에러 (제한 초과 등).
    Error {
        module_id: String,
        code: String,
        message: String,
    },
    /// 지속 시간 임계값 초과 경고 (Q7-2, 제한에 안 걸렸는데 느린 경우).
    Slow {
        module_id: String,
        elapsed_ms: u64,
        threshold_ms: u64,
    },
}

/// 스크립트 실행 옵저버 트레잇.
///
/// 엔진(`SandboxedContext`)이 이 트레잇을 통해 옵저버에 보고한다.
#[async_trait::async_trait]
pub trait ScriptObserver: Send + Sync {
    /// 실행 시작 보고.
    fn on_start(&self, module_id: &str, version: u64);
    /// 실행 종료 보고 (성공).
    fn on_end(&self, report: &ExecutionReport);
    /// 실행 에러 보고.
    fn on_error(&self, module_id: &str, code: &str, message: &str);
}

/// 무작위 옵저버 (기본, 비용 0).
#[derive(Debug, Clone, Default)]
pub struct NoopObserver;

#[async_trait::async_trait]
impl ScriptObserver for NoopObserver {
    fn on_start(&self, _module_id: &str, _version: u64) {}
    fn on_end(&self, _report: &ExecutionReport) {}
    fn on_error(&self, _module_id: &str, _code: &str, _message: &str) {}
}

/// 로거 옵저버: 표준출력으로 이벤트 기록.
/// Phase 18: JSON structured logging 옵션.
#[derive(Debug, Clone)]
pub struct LoggingObserver {
    slow_threshold_ms: u64,
    json_mode: bool,
}

impl LoggingObserver {
    pub fn new(slow_threshold_ms: u64) -> Self {
        Self {
            slow_threshold_ms,
            json_mode: false,
        }
    }

    /// JSON structured logging 모드 (Phase 18).
    pub fn json(slow_threshold_ms: u64) -> Self {
        Self {
            slow_threshold_ms,
            json_mode: true,
        }
    }
}

#[async_trait::async_trait]
impl ScriptObserver for LoggingObserver {
    fn on_start(&self, module_id: &str, version: u64) {
        if self.json_mode {
            let log = serde_json::json!({
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "level": "INFO",
                "message": "hook_start",
                "module_id": module_id,
                "version": version,
            });
            println!("{}", log.to_string());
        } else {
            println!("[OBS] start module={module_id} version={version}");
        }
    }

    fn on_end(&self, report: &ExecutionReport) {
        if self.json_mode {
            let log = serde_json::json!({
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "level": "INFO",
                "message": "hook_end",
                "module_id": report.module_id,
                "elapsed_ms": report.elapsed.as_millis() as u64,
                "exceeded": report.exceeded,
                "error_code": report.error_code,
                "memory_peak": report.memory_peak,
            });
            println!("{}", log.to_string());
        } else if report.is_slow(self.slow_threshold_ms) {
            println!(
                "[OBS] SLOW module={} elapsed_ms={} (threshold={})",
                report.module_id,
                report.elapsed.as_millis(),
                self.slow_threshold_ms
            );
        } else {
            println!(
                "[OBS] end module={} elapsed_ms={}",
                report.module_id,
                report.elapsed.as_millis()
            );
        }
    }

    fn on_error(&self, module_id: &str, code: &str, message: &str) {
        if self.json_mode {
            let log = serde_json::json!({
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "level": "ERROR",
                "message": "hook_error",
                "module_id": module_id,
                "error_code": code,
                "error_message": message,
            });
            println!("{}", log.to_string());
        } else {
            println!("[OBS] ERROR module={module_id} code={code} msg={message}");
        }
    }
}

/// 이벤트 큐 옵저버: `mpsc` 채널로 이벤트를 전파.
///
/// Phase 4/5에서 Prometheus/알림 시스템이 이 채널을 소비.
pub struct EventQueueObserver {
    tx: mpsc::Sender<EvalEvent>,
    slow_threshold_ms: u64,
}

impl EventQueueObserver {
    /// 채널 + 옵저버 생성. `rx`를 반환해 소비자(로거/메트릭)가 수신.
    pub fn new(slow_threshold_ms: u64, capacity: usize) -> (Self, mpsc::Receiver<EvalEvent>) {
        let (tx, rx) = mpsc::channel(capacity);
        (
            Self {
                tx,
                slow_threshold_ms,
            },
            rx,
        )
    }

    /// 지속 시간 임계값(ms).
    pub fn slow_threshold_ms(&self) -> u64 {
        self.slow_threshold_ms
    }
}

#[async_trait::async_trait]
impl ScriptObserver for EventQueueObserver {
    fn on_start(&self, module_id: &str, version: u64) {
        let _ = self.tx.try_send(EvalEvent::Start {
            module_id: module_id.to_string(),
            version,
        });
    }

    fn on_end(&self, report: &ExecutionReport) {
        let _ = self.tx.try_send(EvalEvent::End {
            module_id: report.module_id.clone(),
            report: report.clone(),
        });
        if report.is_slow(self.slow_threshold_ms) {
            let _ = self.tx.try_send(EvalEvent::Slow {
                module_id: report.module_id.clone(),
                elapsed_ms: report.elapsed.as_millis() as u64,
                threshold_ms: self.slow_threshold_ms,
            });
        }
    }

    fn on_error(&self, module_id: &str, code: &str, message: &str) {
        let _ = self.tx.try_send(EvalEvent::Error {
            module_id: module_id.to_string(),
            code: code.to_string(),
            message: message.to_string(),
        });
    }
}

/// 옵저버 + 실행 보고 헬퍼.
///
/// `SandboxedContext`가 eval 호출 시 이 헬퍼를 사용해
/// 이벤트 + ExecutionReport를 자동으로 생성/보고.
pub struct Observability {
    pub observer: Arc<dyn ScriptObserver>,
    pub slow_threshold_ms: u64,
}

impl Observability {
    pub fn new(observer: Arc<dyn ScriptObserver>, slow_threshold_ms: u64) -> Self {
        Self {
            observer,
            slow_threshold_ms,
        }
    }

    /// 노옵 옵저버 (기본, 비용 0).
    pub fn noop() -> Self {
        Self {
            observer: Arc::new(NoopObserver),
            slow_threshold_ms: 0,
        }
    }

    /// 모듈 실행 시작 보고.
    pub fn start(&self, module_id: &str, version: u64) {
        self.observer.on_start(module_id, version);
    }

    /// 실행 종료 보고 + ExecutionReport 반환.
    pub fn finish_success(
        &self,
        module_id: &str,
        version: u64,
        start: std::time::Instant,
        memory_peak: usize,
    ) -> ExecutionReport {
        let end = std::time::Instant::now();
        let elapsed = end.duration_since(start);
        let report = ExecutionReport {
            module_id: module_id.to_string(),
            version,
            start,
            end,
            elapsed,
            exceeded: false,
            error_code: None,
            memory_peak,
        };
        self.observer.on_end(&report);
        report
    }

    /// 실행 에러 보고 + ExecutionReport 반환.
    pub fn finish_error(
        &self,
        module_id: &str,
        version: u64,
        start: std::time::Instant,
        err: &InfraError,
        memory_peak: usize,
    ) -> ExecutionReport {
        let end = std::time::Instant::now();
        let elapsed = end.duration_since(start);
        let code = err.code().to_string();
        let message = err.message().to_string();
        self.observer.on_error(module_id, &code, &message);
        ExecutionReport {
            module_id: module_id.to_string(),
            version,
            start,
            end,
            elapsed,
            exceeded: true,
            error_code: Some(code),
            memory_peak,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// ExecutionReport.is_slow 경계값.
    #[test]
    fn report_is_slow_boundary() {
        let start = std::time::Instant::now();
        let end = start + Duration::from_millis(150);
        let report = ExecutionReport {
            module_id: "m".into(),
            version: 1,
            start,
            end,
            elapsed: end.duration_since(start),
            exceeded: false,
            error_code: None,
            memory_peak: 0,
        };
        assert!(report.is_slow(100));
        assert!(!report.is_slow(200));
    }

    /// 노옵 옵저버가 패닉하지 않는지.
    #[test]
    fn noop_observer_safe() {
        let obs = NoopObserver;
        obs.on_start("m", 1);
        obs.on_error("m", "CJ1102", "timeout");
    }

    /// Observability 헬퍼가 리포트를 생성하는지.
    #[test]
    fn observability_reports() {
        let obs = Observability::noop();
        let start = std::time::Instant::now();
        let report = obs.finish_success("m", 1, start, 1024);
        assert!(!report.exceeded);
        assert_eq!(report.module_id, "m");

        let err_report = obs.finish_error("m", 1, start, &InfraError::Cj102, 2048);
        assert!(err_report.exceeded);
        assert_eq!(err_report.error_code.as_deref(), Some("CJ1102"));
    }
}
