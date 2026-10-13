# rquickjs 0.14 API 변경점 (Phase 1 구현 기준)

> rquickjs **0.14.0** (2026-09-30 조사) 기준으로, Phase 1(인프라 뼈대: 런타임 초기화 + Throttling)
> 구현에 필요한 API의 정확한 시그니처와 구버전 대비 변경점을 기록한다.
> 모든 시그니처는 실제로 빌드/동작을 검증한 것이다. 빌드 환경은 `docs/setting/BUILD_ENV_SETUP.md`.

---

## 1. 모듈 구성

0.14는 단일 `rquickjs`가 아닌 서브크레이트 구조다.

| 크레이트 | 역할 |
|----------|------|
| `rquickjs` | 재수출 facade (`Runtime`, `Context`, `Error`, ...) |
| `rquickjs-core` | 실제 구현 (`Runtime`, `Context`, `Ctx`, `Value`, ...) |
| `rquickjs-sys` | QuickJS C FFI 바인딩 (MSVC `cl.exe`로 QuickJS C 컴파일) |
| `rquickjs-macro` | `#[class]` 매크로 (features `classes` 필요) |

**필요한 features:** `classes`(JS 클래스 바인딩), `futures`(AsyncRuntime, Phase 3 비동기 훅용).
`json` feature는 0.14에 **없다**(구버전과 다름). `json!` 매크로는 별도 `rquickjs-json` 또는 수동 `Value` 생성.

---

## 2. 런타임 생성 (Runtime)

```rust
use rquickjs::Runtime;
let rt = Runtime::new()?;          // -> Result<Runtime, Error>  (패닉 없음, Result 반환)
```

구버전에서 `Runtime::new()`가 `Runtime`을 그대로 반환했던 코드라면
0.14에서는 `Result<Runtime>`이라 `?` 또는 매핑이 필요하다.

---

## 3. Throttling API (핵심 변경)

### 3.1 메모리 제한 (CJ1103 매핑)

```rust
rt.set_memory_limit(limit: usize); // 바이트 단위. 0 = 무제한
```
- 구버전의 `Option<u64>` 방식이 **아님**. 0.14는 `usize` 단 하나의 값.
- **주의:** `rust-alloc` 또는 `allocator` feature를 쓰면 이 API는 **No-op**이다.
  Phase 1은 기본(QuickJS 내장 할당기) 할당 기준으로 사용해야 제한이 실제로 적용된다.
- 제한 초과 시 `eval`이 `Error::Exception`(`is_exception()==true`)을 반환한다.

### 3.2 스택 제한

```rust
rt.set_max_stack_size(limit: usize); // 기본 256 * 1024 바이트
```

### 3.3 GC 임계값

```rust
rt.set_gc_threshold(threshold: usize); // 0 = GC 자동
```

### 3.4 메모리 사용량 조회 (모니터링용)

```rust
let usage = rt.memory_usage(); // -> MemoryUsage (= qjs::JSMemoryUsage)
usage.malloc_limit;            // i64: 설정한 제한
usage.malloc_size;             // i64: 현재 malloc 사용량
usage.memory_used_size;        // i64: 실제 사용 메모리
```

### 3.5 CPU 타임아웃 (CJ1102 매핑) — **단일 API가 없음**

0.14에는 `set_cpu_timeout(ms)` 같은 전용 API가 **없다**.
`InterruptHandler`(클로저)로 구현한다.

```rust
use std::time::Instant;
let start = Instant::now();
rt.set_interrupt_handler(Some(Box::new(move || start.elapsed().as_millis() >= 500)));
// ... 스크립트 실행 ...
rt.set_interrupt_handler(None); // 해제
```

- `InterruptHandler = Box<dyn FnMut() -> bool + 'static>`
- 핸들러가 `true`를 반환하면 인터프리터가 **언캐치 가능한 예외**를 던지고 제어권을 반환.
- 제한 시간은 `start.elapsed()`로 클로저가 자체 판정.
- **주의:** `futures` feature 사용 시 `InterruptHandler`에 `+ Send`가 요구된다.

---

## 4. 컨텍스트 (Context)

```rust
use rquickjs::Context;
let ctx = Context::full(&rt)?;   // -> Result<Context, Error>
```

- `Context::full(&runtime)`이 컨텍스트 생성 진입점이다. (구버전의 `ctx.new_context()` 등 다름)
- `ctx.with(|ctx: Ctx| { ... })`으로 `Ctx`를 받아야 JS API 접근.
  `Ctx`는 `with` 클로저 안에서만 유효 (레포를 락하지 못하도록 설계).
- `ctx.runtime()`는 **없다**. 런타임에 접근하려면 원래 `&Runtime`을 직접 참조.

### eval

```rust
ctx.with(|ctx| {
    let v: f64 = ctx.eval::<f64, _>("1 + 1")?;   // -> Result<V, Error>
    Ok::<f64, Error>(v)
});
```

---

## 5. 에러 분류 (CJ1102 / CJ1103 매핑의 근거)

```rust
use rquickjs::Error;
// OOM 또는 인터럽트 모두 -> Error::Exception
assert!(err.is_exception());   // true
```

- 메모리 초과(OOM)와 CPU 타임아웃(interrupt)은 둘 다 **`Error::Exception`**으로 surfaces한다.
- 즉, `err.is_exception()`만으로는 OOM과 CPU 타임아웃을 **분리할 수 없다**.
- **구분 방법(Phase 1 design):**
  1. `memory_usage().malloc_size >= malloc_limit`이면 CJ1103(메모리 초과)
  2. 인터럽트 핸들러가 실제 초과를 감지했으면(플래그) CJ1102(CPU 초과)
  3. 둘 다 아니면 일반 JS 예외(CJ1101 계열 or 스크립트 throw)
- `CaughtError::from_error(ctx, error)`로 스크립트가 `throw new Error(...)`한
  실제 JS 값을 가져와 JS 내부 에러코드(JSON)와 인프라 에러를 구분할 수 있다.

---

## 6. JS에서 Rust로 예외 전달 (Phase 2/3 대비)

```rust
// Rust 바인딩 함수가 JS에 에러 던지기
use rquickjs::Exception;
fn my_fn(ctx: Ctx<'_>) -> rquickjs::Result<()> {
    // JS 측에서 try/catch로 {code, message}를 받을 수 있는 형식
    let ex = Exception::from_message(ctx, "SJ1202: 세전금액은 0 이상이어야 합니다.")?;
    Err(ctx.throw(ex))  // -> Error
}
```

- `Exception::from_message(ctx, &str)`은 **메시지 문자열** 기반.
  0.14에는 `json!` 매크로(feature `json` 없음)가 없으므로,
  구조화된 `{code, message}` 객체를 만들려면 `Object`/`Array` API로 직접 구성하거나
  메시지 문자열에 JSON을 serialize해 넣는 방식을 택한다.

---

## 7. 구버전 대비 주요 Breaking Change 요약

| 항목 | 구버전 (예상) | 0.14.0 |
|------|---------------|--------|
| `Runtime::new()` | `-> Runtime` | `-> Result<Runtime>` |
| 컨텍스트 생성 | `runtime.new_context()` | `Context::full(&runtime)?` |
| 메모리 제한 | `Option<u64>` | `set_memory_limit(usize)` (0=무제한) |
| CPU 타임아웃 | `set_cpu_timeout(ms)` 존재 (부정확) | **전용 API 없음**, `set_interrupt_handler`로 구현 |
| `json!` 매크로 | feature `json` | **feature 없음** (별도 처리 필요) |
| `Ctx` 접근 | 직접 borrow | `ctx.with(\|ctx\| ...)` 클로저 필수 |
| `ctx.runtime()` | 존재 | **없음** |

> 이 표는 "구버전"열이 0.14 이전의 일반적인 패턴을 정리한 것으로,
> Phase 1 코딩 시 0.14의 **오른쪽** 시그니처를 정확히 따라야 한다.
