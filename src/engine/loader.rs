//! 스크립트 모듈 로딩 파이프라인: `scripts/base` → `extensions`.
//!
//! - 기준(Base) 모듈과 확장(Extension) 모듈을 `DataStore`(L3)에서 적재
//! - JS 소스를 rquickjs에 컴파일 (CJ1101 구문오류 감지)
//! - Base→Extension 상속 구조를 보장 (확장은 기준 모듈을 상속)
//! - 컴파일된 스크립트 스냅샷을 `SandboxedContext`에 바인딩
//!
//! AGENTS.md 체크리스트:
//! - [ ] 컴파일/배포 없이 DB/Redis 데이터 변경만으로 동작
//! - [ ] 독립된 샌드박스 컨텍스트 보장 (Base 모듈 데이터 스코프 오염 방지)
//! - [ ] 에러 코드 명명 규칙 준수 (CJ1101, CJ1102, CJ1103, CJ1104)

use crate::core::error::InfraError;
use crate::engine::{Engine, SandboxedContext, ThrottleConfig};
use crate::meta::store::{DataStore, MetaModule};

/// 로딩된 모듈의 컴파일된 상태.
struct CompiledModule {
    #[allow(dead_code)]
    module_id: String,
    #[allow(dead_code)]
    kind: String,
    #[allow(dead_code)]
    version: u64,
}

/// 모듈 로더: DataStore + Engine을 결합해 스크립트 파이프라인을 구성.
///
/// `engine`: 실제 실행 엔진 (CPU Timeout 적용)
/// `compile_engine`: 구문 검증용 엔진 (CPU Timeout 없음, interrupt handler 재설정 방지)
pub struct ModuleLoader {
    engine: Engine,
    compile_engine: Engine,
}

/// 모듈 로딩/컴파일 에러.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("CJ1101: {0}")]
    Syntax(String),
    #[error("CR0002: 엔진/컨텍스트 생성 실패")]
    Engine,
    #[error("CJ1102: {0}")]
    CpuTimeout(String),
    #[error("CJ1103: {0}")]
    Memory(String),
}

/// Base→Extension 파이프라인에 바인딩될 훅 이름들.
///
/// AGENTS.md에 정의된 라이프사이클 훅.
pub const HOOK_NAMES: &[&str] = &[
    "before_request",
    "after_save",
    "before_create",
    "before_update",
    "before_delete",
    "after_request",
];

impl ModuleLoader {
    pub fn new(engine: Engine, _throttle_cfg: ThrottleConfig) -> Self {
        // 구문 검증용 엔진: cpu_timeout_ms=0 (interrupt handler 없음)
        // 공유 Runtime의 interrupt handler가 재설정되지 않도록.
        let compile_engine = Engine::new(ThrottleConfig {
            memory_limit: 8 * 1024 * 1024,
            max_stack: 0,
            cpu_timeout_ms: 0,
        })
        .expect("구문 검증 엔진 생성");
        Self {
            engine,
            compile_engine,
        }
    }

    /// 활성화된 모든 모듈을 DataStore에서 적재하고 컴파일한다.
    ///
    /// CJ1101(구문오류)이 발생한 모듈은 로딩을 중단하고 에러 반환.
    /// (AGENTS.md: 결정적 실패 보호 — 안전한 버전으로 후퇴)
    pub async fn load_modules(&self, store: &dyn DataStore) -> Result<Vec<MetaModule>, LoadError> {
        let modules = store
            .list_enabled_modules()
            .await
            .map_err(|e| LoadError::Syntax(format!("L3 조회 실패: {e}")))?;

        // Base 모듈 먼저, Extension 나중에 (의존성 순서)
        let mut base_modules: Vec<&MetaModule> =
            modules.iter().filter(|m| m.kind == "base").collect();
        base_modules.sort_by(|a, b| a.module_id.cmp(&b.module_id));
        let mut ext_modules: Vec<&MetaModule> =
            modules.iter().filter(|m| m.kind == "extension").collect();
        ext_modules.sort_by(|a, b| a.module_id.cmp(&b.module_id));

        // 각 모듈 컴파일 (CJ1101 검증을 위해)
        for m in base_modules.iter().chain(ext_modules.iter()) {
            self.compile_and_validate(&m.script_source)?;
        }

        Ok(modules)
    }

    /// Base 모듈의 훅을 확장 컨텍스트에 바인딩하고,
    /// 확장 스크립트를 실행해 훅 체이닝을 구성한다.
    ///
    /// 반환: 확장 모듈에 바인딩된 샌드박스 컨텍스트 (훅 호출 가능).
    pub fn bind_extension(
        &self,
        base_module: &MetaModule,
        ext_module: &MetaModule,
    ) -> Result<SandboxedContext, LoadError> {
        let sb = self
            .engine
            .create_sandbox()
            .map_err(|_| LoadError::Engine)?;

        // 1. Base 스크립트 실행 (훅 정의)
        // Base: IIFE가 api를 정의하고 반환.
        // Extension: Base를 프로토타입으로, ext 스크립트가 `api` 인자에 직접 훅을 정의.
        // (ext 스크립트는 `var api`를 선언하지 않아야 함 — 인자 api에 직접 작업)
        let base_script = format!(
            r#"
            var Base = (function(){{
                var api = {{}};
                {};
                return api;
            }})();
            globalThis.Base = Base;
            var Extension = (function(){{
                var proto = Object.create(Base);
                (function(api){{
                    {};
                    return api;
                }})(proto);
                return proto;
            }})();
            globalThis.Extension = Extension;
            1
            "#,
            base_module.script_source, ext_module.script_source
        );

        // 2. 컴파일 검증 (CJ1101)
        self.compile_and_validate(&base_script)?;

        // 3. 실행 (훅 체이닝 구성)
        sb.eval_i64(&base_script).map_err(|e| match e {
            InfraError::Cj102 => LoadError::CpuTimeout(e.message().to_string()),
            InfraError::Cj103 => LoadError::Memory(e.message().to_string()),
            _ => LoadError::Syntax(e.message().to_string()),
        })?;

        Ok(sb)
    }

    /// JS 소스를 컴파일해 구문오류(CJ1101)를 검증한다 (테스트용 공개).
    pub fn compile_validate_public(&self, source: &str) -> Result<(), LoadError> {
        self.compile_and_validate(source)
    }

    /// JS 소스를 컴파일해 구문오류(CJ1101)를 검증한다.
    ///
    /// `eval`로 함수 표현식을 평가해 구문 오류가 즉시 검출된다.
    /// 구문 오류이면 `LoadError::Syntax`(CJ1101) 반환.
    fn compile_and_validate(&self, source: &str) -> Result<(), LoadError> {
        let sb = self
            .compile_engine
            .create_sandbox()
            .map_err(|_| LoadError::Engine)?;

        // 함수 체로 감싸 컴파일 (구문 오류 검출)
        let wrapped = format!("(function(){{ {source} }})");
        // i64로 평가 — 구문 오류가 있으면 eval 실패
        let res: Result<i64, InfraError> = sb.eval_i64(&format!("{wrapped}; 1"));
        match res {
            Ok(_) => Ok(()),
            Err(e) => {
                let msg = e.message().to_string();
                let lower = msg.to_lowercase();
                // 구문 오류 특징 문자열 또는 일반 예외 모두 CJ1101(캐시 적재 단계 차단)
                let _ = lower;
                Err(LoadError::Syntax(msg))
            }
        }
    }

    /// 특정 훅 이름이 바인딩된지 확인 (테스트/검증용).
    pub fn has_hook(&self, sb: &SandboxedContext, hook_name: &str) -> bool {
        let check = format!(
            "typeof Extension !== 'undefined' && typeof Extension['{}'] === 'function' ? 1 : 0",
            hook_name
        );
        sb.eval_i64(&check).unwrap_or(0) == 1
    }
}

impl ModuleLoader {
    /// Engine accessor (테스트용).
    pub fn engine(&self) -> &Engine {
        &self.engine
    }
}
