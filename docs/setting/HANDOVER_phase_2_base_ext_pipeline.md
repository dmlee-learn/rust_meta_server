# Phase 2 이어서 개발하기 (Binding Layer Handover)

> **대상:** Phase 2 (바인딩 레이어)를 **중단했다가 이어서 진행하는** 누구든.
> Phase 1(런타임 초기화 + Throttling)이 완료된 상태에서 이어서 진행한다.

---

## 1. Phase 2 목표 (Goal)

JS 스크립트 내부에서 `rust_` 접두사 함수를 호출할 수 있도록, Rust 고성능 함수(SDK)를 rquickjs 컨텍스트에 안전하게 주입(Binding)하는 구조를 완성하는 것이다.

**핵심 원칙 (AGENTS.md 준수):**
- 모든 바인딩 함수는 `rust_` 접두사 필수 (`rust_calculate_base_tax()` 등)
- **패닉 절대 금지:** `unwrap()`, `expect()`, `panic!` 사용 시 즉시 에러
- 반드시 `Result<T, rquickjs::Error>` 반환
- JS 측에서 예외 발생 시 JSON 에러 코드 객체 반환 (`{"code":"SJ1202","message":"..."}`)

## 2. 기대 폴더 구조 (Expected Layout)

```
src/
├── engine/
│   └── mod.rs        # Phase 1에서 완성된 런타임 생성기
├── sdk/
│   ├── mod.rs        # SDK 진입점, 바인딩 루틴
│   ├── math.rs       # 계산 계열 (rust_calculate_base_tax 등)
│   └── utils.rs      # 유틸 계열 (rust_hash_token 등)
```

## 3. 핵심 구현 스펙 (Implementation Spec)

### 3.1 SDK 함수 작성 규칙

```rust
// src/sdk/math.rs
use rquickjs::Result;

/// 부가가치세 기본세율 계산 (세전금액 기준)
/// - 세전금액: 양수 실수
/// - 결과: 10% 부가세 포함 총액
/// - 에러: 음수 입력 시 SJ1202 (스크립트 계산 오류 - 비즈니스 룰 위반)
pub fn rust_calculate_base_tax(amount: f64) -> Result<f64> {
    if amount < 0.0 {
        // JS 에러 객체로 변환하여 반환 (패닉 금지)
        return Err(rquickjs::Error::from_value(
            rquickjs::Exception::from(
                rquickjs::json::json!({"code": "SJ1202", "message": "세전금액은 0 이상이어야 합니다."})
            )
        ));
    }
    Ok(amount * 1.1)
}

/// 토큰 해싱 (HMAC-SHA256 기반)
pub fn rust_hash_token(token: String, secret: String) -> Result<String> {
    // 실제 구현: hmac sha256 라이브러리 사용
    // 빈 입력 검증 포함
    if token.is_empty() {
        return Err(rquickjs::Error::from_value(
            rquickjs::Exception::from(
                rquickjs::json::json!({"code": "SJ1203", "message": "토큰이 비어있습니다."})
            )
        ));
    }
    // ... HMAC 계산 로직
    Ok("hashed_result".to_string()) // placeholders
}
```

### 3.2 컨텍스트 주입 (Binding) 루틴

```rust
// src/sdk/mod.rs
use rquickjs::{Runtime, Context};
use std::sync::Arc;

/// JS 컨텍스트에 모든 SDK 함수를 주입한다.
/// Phase 1에서 생성된 런타임을 받아, 해당 컨텍스트에 바인딩을 수행한다.
pub fn bind_sdk_to_context(rt: &Arc<Runtime>) -> Result<Context> {
    let ctx = rt.new_context().map_err(|e| {
        rquickjs::Error::from_value(rquickjs::Exception::from(
            rquickjs::json::json!({"code": "CR0002", "message": "Context creation failed"})
        ))
    })?;

    ctx.with(|ctx| {
        // 함수 바인딩 (rust_ 접두사 유지)
        let globals = ctx.globals()?;
        
        globals.set("rust_calculate_base_tax", 
            rquickjs::function(ctx.clone(), math::rust_calculate_base_tax))?;
        
        globals.set("rust_hash_token", 
            rquickjs::function(ctx.clone(), utils::rust_hash_token))?;
        
        Ok(())
    })?;

    Ok(ctx)
}
```

### 3.3 JS 스크립트에서 호출 예시

```javascript
// scripts/base/tax_module.js (예시)
function calculateTotalPrice(basePrice) {
    // Rust의 고성능 계산 함수 호출
    const taxedPrice = rust_calculate_base_tax(basePrice);
    return taxedPrice;
}

try {
    const result = calculateTotalPrice(-100);
} catch (e) {
    // e는 {code: "SJ1202", message: "..."} JSON 객체
    console.log(e.code); // "SJ1202"
}
```

## 4. 검증 테스트 (Verification)

| 테스트 | 스크립트 | 기대 결과 |
|--------|----------|-----------|
| 정상 계산 | `rust_calculate_base_tax(10000)` | `11000` 반환 |
| 음수 에러 | `rust_calculate_base_tax(-100)` | `{code:"SJ1202", message:"..."}` 예외 |
| 패닉 확인 | `cargo test` | `unwrap`/`panic` 발생 없음 |
| 함수 존재 | `typeof rust_hash_token` | `"function"` 반환 |

## 5. 단절 시 체크포인트 기록 양식

```
### 현재 상태
- 진행 중 Task: P2-2.2 (컨텍스트 주입 구조)
- 중단 위치: src/sdk/mod.rs, bind_sdk_to_context() 34번째 라인
- 다음 액션:
  1. utils::rust_hash_token 바인딩 추가
  2. tests/sdk_binding.rs에서 2개 함수 모두 호출 테스트 작성
- 주의사항:
  - rquickjs::function() 제네릭 타입 추론 실패 시 명시적 타입 지정 필요
  - Arc<Runtime>와 Context 생명주기 관리 주의 (Context는 Runtime보다 먼저 폐기되면 안 됨)
```

## 6. 다음 단계 (Handover to Phase 3)

Phase 2 완료 후 `docs/STATUS.md` 갱신:

```
### 현재 상태
- 진행 중 Task: P2 완료 - Phase 3.1 (L1 캐시 적재기) 착수 대기
- 다음 액션:
  1. src/meta/ 폴더에 L1 인메모리 캐시 구조체 설계
  2. 스크립트 로드 로직 (Redis → L1) 구현
  3. Single Flight 패턴 적용
```
