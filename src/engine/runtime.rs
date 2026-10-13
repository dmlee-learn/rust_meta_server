//! rquickjs 런타임 초기화 및 Throttling 강제.
//!
//! `Engine`은 서버 기동 시 1회 생성되고, `SandboxedContext`는 요청별로
//! 생성되는 샌드박스 컨텍스트를 표현한다.

use rquickjs::{Context, Runtime};
use std::sync::Arc;
use std::time::Instant;

use super::throttle::{CpuExceeded, ThrottleConfig};
use crate::core::error::InfraError;
use crate::observability::Observability;

/// 샌드박스 런타임 한 개 + Throttling 설정을 묶은 엔진.
///
/// `Arc`로 공유되어 Axum 핸들러에서 요청별 컨텍스트를 만든다.
#[derive(Clone)]
pub struct Engine {
    pub(crate) rt: Arc<Runtime>,
    pub(crate) cfg: ThrottleConfig,
}

/// 요청별로 생성되는 샌드박스 컨텍스트.
///
/// 런타임의 CPU Timeout / Memory Limit이 이 컨텍스트의 모든 eval에 적용된다.
pub struct SandboxedContext {
    pub(crate) ctx: Context,
    pub(crate) cpu_exceeded: Arc<CpuExceeded>,
    /// 모듈 ID (옵저버 보고용, Q5).
    pub module_id: String,
    /// 모듈 버전 (옵저버 보고용).
    pub version: u64,
    /// 옵저버 헬퍼 (Q5-3).
    pub observability: Option<Arc<Observability>>,
}

impl Engine {
    /// 서버 기동 시 호출.
    ///
    /// - rquickjs Runtime 생성
    /// - Memory Limit, Max Stack, GC Threshold 강제 적용
    ///
    /// 패닉하지 않으며, 실패 시 `Result::Err(InfraError)` 반환.
    pub fn new(cfg: ThrottleConfig) -> Result<Self, InfraError> {
        let rt = Runtime::new().map_err(|_| InfraError::Cr002)?;

        // 메모리 제한 강제 (CJ1103 방어)
        rt.set_memory_limit(cfg.memory_limit);

        // 스택 제한 강제
        if cfg.max_stack > 0 {
            rt.set_max_stack_size(cfg.max_stack);
        }

        // GC 임계값 기본 (0 = 자동)
        rt.set_gc_threshold(0);

        Ok(Self {
            rt: Arc::new(rt),
            cfg,
        })
    }

    /// 현재 설정의 메모리 한도 (바이트).
    pub fn memory_limit(&self) -> usize {
        self.cfg.memory_limit
    }

    /// Phase 34: Engine warm-up — 기동 시 rquickjs JIT prewarm.
    ///
    /// 가벼운 스크립트를 eval하여 런타임 초기화 비용을 최소화.
    /// 첫 요청 latency를 감소한다.
    pub fn warm_up(&self) -> Result<(), InfraError> {
        let sandbox = self.create_sandbox()?;
        sandbox.eval_void("1 + 1")?;
        Ok(())
    }

    /// 현재 설정의 CPU 타임아웃 (ms).
    pub fn cpu_timeout_ms(&self) -> u64 {
        self.cfg.cpu_timeout_ms
    }

    /// 런타임 메모리 사용량 (모니터링/테스트용).
    pub fn memory_usage(&self) -> rquickjs::qjs::JSMemoryUsage {
        self.rt.memory_usage()
    }

    /// GC 수동 트리거 (메모리 회수, 결정적 실패 보호용).
    pub fn force_gc(&self) {
        self.rt.run_gc();
    }

    /// 요청별 샌드박스 컨텍스트 생성.
    ///
    /// `cfg.cpu_timeout_ms > 0`이면 CPU Timeout 인터럽트 핸들러가
    /// 이 컨텍스트의 Runtime에 일시적으로 적용된다.
    pub fn create_sandbox(&self) -> Result<SandboxedContext, InfraError> {
        let ctx = Context::full(self.rt.as_ref()).map_err(|_| InfraError::Cr002)?;
        let cpu_exceeded = Arc::new(CpuExceeded::default());

        // CPU Timeout: interrupt handler로 구현 (rquickjs 0.14)
        if self.cfg.cpu_timeout_ms > 0 {
            let flag = cpu_exceeded.clone();
            let limit_ms = self.cfg.cpu_timeout_ms;
            let start = Instant::now();
            self.rt.set_interrupt_handler(Some(Box::new(move || {
                let exceeded = start.elapsed().as_millis() as u64 >= limit_ms;
                if exceeded {
                    flag.mark();
                }
                exceeded
            })));
        }

        // SDK 함수 바인딩 (rust_ 접두사)
        ctx.with(|ctx| {
            let globals = ctx.globals();
            globals.set("rust_calculate_base_tax", rquickjs::Function::new(ctx.clone(), |_ctx: rquickjs::Ctx, a: f64, r: f64| -> rquickjs::Result<f64> {
                Ok(crate::sdk::math::rust_calculate_base_tax(a, r))
            }))?;
            globals.set("rust_calculate_discount", rquickjs::Function::new(ctx.clone(), |_ctx: rquickjs::Ctx, p: f64, d: f64| -> rquickjs::Result<f64> {
                Ok(crate::sdk::math::rust_calculate_discount(p, d))
            }))?;
            globals.set("rust_hash_token", rquickjs::Function::new(ctx.clone(), |_ctx: rquickjs::Ctx, t: String, s: String| -> rquickjs::Result<String> {
                Ok(crate::sdk::utils::rust_hash_token(t, s))
            }))?;
            globals.set("rust_validate_uuid", rquickjs::Function::new(ctx.clone(), |_ctx: rquickjs::Ctx, u: String| -> rquickjs::Result<bool> {
                Ok(crate::sdk::utils::rust_validate_uuid(u))
            }))?;
            Ok::<(), rquickjs::Error>(())
        }).map_err(|_| InfraError::Cr002)?;

        Ok(SandboxedContext {
            ctx,
            cpu_exceeded,
            module_id: String::new(),
            version: 0,
            observability: None,
        })
    }

    /// 옵저버가 연결된 샌드박스 컨텍스트 생성 (Q5).
    ///
    /// 모듈 ID/버전 + 옵저버 헬퍼를 바인딩해, `eval_observed` 호출 시
    /// 이벤트를 보고한다.
    pub fn create_sandbox_observed(
        &self,
        module_id: &str,
        version: u64,
        obs: Arc<Observability>,
    ) -> Result<SandboxedContext, InfraError> {
        let sb = self.create_sandbox()?;
        Ok(SandboxedContext {
            ctx: sb.ctx,
            cpu_exceeded: sb.cpu_exceeded,
            module_id: module_id.to_string(),
            version,
            observability: Some(obs),
        })
    }
}

impl SandboxedContext {
    /// JS 스크립트를 단순 실행 (반환값 무시).
    ///
    /// 훅 스크립트 바인딩 등 반환값이 필요 없는 스크립트용.
    /// 실패 시 `Err(InfraError)`를 반환.
    pub fn eval_void(&self, source: &str) -> Result<(), InfraError> {
        let res: Result<(), rquickjs::Error> = self.ctx.with(|ctx| {
            let _v: rquickjs::Value = ctx.eval(source)?;
            Ok(())
        });

        match res {
            Ok(_) => Ok(()),
            Err(e) => {
                let usage = self.ctx.runtime().memory_usage();
                let memory_exceeded = usage.malloc_limit > 0
                    && usage.malloc_size as f64 >= usage.malloc_limit as f64 * 0.9;
                let code = super::throttle::classify_error(&e, &self.cpu_exceeded, memory_exceeded);
                self.reset_interrupt();
                Err(code)
            }
        }
    }

    /// JS 스크립트를 i64로 실행 (Phase 1 기본).
    ///
    /// 실패 시 `Err(InfraError)`를 반환하며, CPU Timeout 여부 플래그를
    /// 내부에 보존한다.
    pub fn eval_i64(&self, source: &str) -> Result<i64, InfraError> {
        let res: Result<i64, rquickjs::Error> = self.ctx.with(|ctx| {
            let v: i64 = ctx.eval::<i64, _>(source)?;
            Ok(v)
        });

        match res {
            Ok(v) => Ok(v),
            Err(e) => {
                // 메모리 한계 초과 여부: 컨텍스트의 런타임 malloc 사용량이
                // 한도의 90% 이상이면 메모리 압력으로 간주 (OOM 전후).
                // 순수 JS throw는 malloc_size가 낮으므로 CJ1104로 분류.
                let usage = self.ctx.runtime().memory_usage();
                let memory_exceeded = usage.malloc_limit > 0
                    && usage.malloc_size as f64 >= usage.malloc_limit as f64 * 0.9;
                let code = super::throttle::classify_error(&e, &self.cpu_exceeded, memory_exceeded);
                self.reset_interrupt();
                Err(code)
            }
        }
    }

    /// 명시적 CPU 초과 플래그 리셋.
    pub fn reset_interrupt(&self) {
        self.cpu_exceeded.reset();
    }

    /// JS 스크립트를 `Option<String>`으로 실행 (Phase 5: 소켓 훅용).
    ///
    /// `undefined`, `null`은 `Ok(None)` 반환.
    /// `string`은 `Ok(Some(string))` 반환.
    /// `object`, `array`은 `Ok(Some(JSON.stringify(value)))` 반환.
    /// `number`, `boolean`은 `Ok(Some(String(value)))` 반환.
    /// 실패 시 `Err(InfraError)`를 반환.
    pub fn eval_value_as_string(&self, source: &str) -> Result<Option<String>, InfraError> {
        let res: Result<Option<String>, rquickjs::Error> = self.ctx.with(|ctx| {
            let v: Option<String> = ctx.eval(source)?;
            Ok(v)
        });

        match res {
            Ok(v) => Ok(v),
            Err(e) => {
                let usage = self.ctx.runtime().memory_usage();
                let memory_exceeded = usage.malloc_limit > 0
                    && usage.malloc_size as f64 >= usage.malloc_limit as f64 * 0.9;
                let code = super::throttle::classify_error(&e, &self.cpu_exceeded, memory_exceeded);
                self.reset_interrupt();
                Err(code)
            }
        }
    }

    /// CPU Timeout 초과 여부 (마지막 eval 후).
    pub fn cpu_exceeded(&self) -> bool {
        self.cpu_exceeded.is_set()
    }

    /// 컨텍스트 런타임의 현재 메모리 사용량.
    pub fn runtime_usage(&self) -> rquickjs::qjs::JSMemoryUsage {
        self.ctx.runtime().memory_usage()
    }

    /// 옵저버 보고를 포함해 JS 스크립트를 String으로 실행 (Q5).
    ///
    /// `eval_i64`의 String 버전. `eval_observed`와 동일하게 시작/종료/에러를 보고.
    pub fn eval_string_observed(&self, source: &str) -> Result<String, InfraError> {
        let start = Instant::now();
        if let Some(obs) = &self.observability {
            obs.start(&self.module_id, self.version);
        }
        let res: Result<String, rquickjs::Error> = self.ctx.with(|ctx| {
            let v: String = ctx.eval::<String, _>(source)?;
            Ok(v)
        });
        match &res {
            Ok(s) => {
                if let Some(obs) = &self.observability {
                    let mem = self.ctx.runtime().memory_usage().malloc_size as usize;
                    obs.finish_success(&self.module_id, self.version, start, mem);
                }
                Ok(s.clone())
            }
            Err(e) => {
                let usage = self.ctx.runtime().memory_usage();
                let memory_exceeded = usage.malloc_limit > 0
                    && usage.malloc_size as f64 >= usage.malloc_limit as f64 * 0.9;
                let code = super::throttle::classify_error(e, &self.cpu_exceeded, memory_exceeded);
                self.reset_interrupt();
                if let Some(obs) = &self.observability {
                    let mem = self.ctx.runtime().memory_usage().malloc_size as usize;
                    obs.finish_error(&self.module_id, self.version, start, &code, mem);
                }
                Err(code)
            }
        }
    }

    /// 모듈 ID/버전 + 옵저버를 바인딩 (이동 불가 — 자체 소유권 반환).
    ///
    /// `bind_extension` 결과에 옵저버를 연결해 훅 실행 시 이벤트를 보고.
    pub fn with_observability(mut self, module_id: &str, version: u64, obs: Arc<Observability>) -> Self {
        self.module_id = module_id.to_string();
        self.version = version;
        self.observability = Some(obs);
        self
    }

    /// 컨텍스트에 접근 (로더/테스트용).
    pub fn ctx(&self) -> &rquickjs::Context {
        &self.ctx
    }

    /// 옵저버 보고를 포함해 JS 스크립트를 i64로 실행 (Q5).
    ///
    /// `eval_i64`와 동일하지만, 시작/종료/에러를 `Observability`에 보고한다.
    /// 모듈 실행(훅 발동) 시에는 이 메서드를 사용해 무한루프/느린 실행을 감시.
    pub fn eval_observed(&self, source: &str) -> Result<i64, InfraError> {
        let start = Instant::now();
        if let Some(obs) = &self.observability {
            obs.start(&self.module_id, self.version);
        }
        let res = self.eval_i64(source);
        if let Some(obs) = &self.observability {
            let mem_peak = self.ctx.runtime().memory_usage().malloc_size as usize;
            match &res {
                Ok(_) => {
                    obs.finish_success(&self.module_id, self.version, start, mem_peak);
                }
                Err(e) => {
                    obs.finish_error(&self.module_id, self.version, start, e, mem_peak);
                }
            }
        }
        res
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ThrottleConfig;

    #[test]
    fn engine_creates_with_limits() {
        let engine = Engine::new(ThrottleConfig {
            memory_limit: 4 * 1024 * 1024,
            max_stack: 0,
            cpu_timeout_ms: 200,
        })
        .expect("engine 생성 실패 (CR0002)");
        assert_eq!(engine.memory_limit(), 4 * 1024 * 1024);
        assert_eq!(engine.cpu_timeout_ms(), 200);
    }

    #[test]
    fn sandbox_eval_simple() {
        let engine = Engine::new(ThrottleConfig {
            memory_limit: 4 * 1024 * 1024,
            max_stack: 0,
            cpu_timeout_ms: 0, // CPU 제한 테스트를 위함
        })
        .unwrap();
        let sb = engine.create_sandbox().unwrap();
        let v: i64 = sb.eval_i64("1 + 1").unwrap();
        assert_eq!(v, 2);
    }

    #[test]
    fn sandbox_cpu_timeout_triggers_cj102() {
        let engine = Engine::new(ThrottleConfig {
            memory_limit: 64 * 1024 * 1024,
            max_stack: 0,
            cpu_timeout_ms: 100, // 100ms 제한
        })
        .unwrap();
        let sb = engine.create_sandbox().unwrap();
        let res: Result<i64, InfraError> = sb.eval_i64("let s=0;for(let i=0;i<4e9;i++){s+=i}s");
        assert!(res.is_err(), "무한 루프가 CPU Timeout으로 종료되어야 함");
        assert_eq!(res.unwrap_err(), InfraError::Cj102);
    }

    #[test]
    fn sandbox_memory_limit_triggers_cj103() {
        let engine = Engine::new(ThrottleConfig {
            memory_limit: 256 * 1024, // 256KB로 매우 낮게
            max_stack: 0,
            cpu_timeout_ms: 0,
        })
        .unwrap();
        let sb = engine.create_sandbox().unwrap();
        let res: Result<i64, InfraError> =
            sb.eval_i64("var a=[];for(let i=0;i<2000000;i++){a.push('x'.repeat(200));}1");
        assert!(res.is_err(), "OOM 발생해야 함");
        assert_eq!(res.unwrap_err(), InfraError::Cj103);
    }

    #[test]
    fn sandbox_js_exception_maps_to_infra_error() {
        let engine = Engine::new(ThrottleConfig {
            memory_limit: 4 * 1024 * 1024,
            max_stack: 0,
            cpu_timeout_ms: 0,
        })
        .unwrap();
        let sb = engine.create_sandbox().unwrap();
        let res: Result<i64, InfraError> = sb.eval_i64("throw new Error('SJ999: test');1");
        // CPU/메모리 초과가 아닌 일반 JS 예외 -> CJ1104로 분류
        assert_eq!(res.unwrap_err(), InfraError::Cj104);
    }
}
