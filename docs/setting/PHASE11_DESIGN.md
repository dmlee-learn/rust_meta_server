# Phase 11 설계: SDK 구현 + 추가 도메인 모듈

> Rust SDK 함수를 rquickjs에 바인딩하고,
> ERP/Blog/Shop Base+Extension 스크립트를 추가한다.

---

## 1. 설계 결정

### D1: SDK 구조

**결정: `src/sdk/mod.rs` + `src/sdk/math.rs` + `src/sdk/utils.rs`**

```
src/sdk/
├── mod.rs      # SDK 모듈 등록, SandboxedContext에 바인딩
├── math.rs     # 계산 계열 (rust_calculate_base_tax, rust_calculate_discount)
└── utils.rs    # 유틸 계열 (rust_hash_token, rust_validate_uuid)
```

**규약:**
- `rust_` 접두사 필수
- `Result<T, rquickjs::Error>` 반환 (panic 금지)
- 음수/무효 입력 시 에러 코드 JSON 반환

### D2: SDK 함수 시그니처

| 함수 | 시그니처 | 설명 |
|------|----------|------|
| `rust_calculate_base_tax` | `fn(amount: f64, rate: f64) -> Result<f64>` | 원가 + 부가세 계산 |
| `rust_calculate_discount` | `fn(price: f64, discount_pct: f64) -> Result<f64>` | 할인율 적용 |
| `rust_hash_token` | `fn(token: String, secret: String) -> Result<String>` | SHA-256 해시 |
| `rust_validate_uuid` | `fn(uuid: String) -> Result<bool>` | UUID 형식 검증 |

### D3: 추가 도메인 모듈

**ERP Base:**
- `scripts/base/erp_workflow.js` — ERP 워크플로우 Base
  - `before_process`: 입력 검증
  - `after_process`: 후처리

**ERP Extension:**
- `scripts/extensions/erp_purchase_order.js` — 발주서 워크플로우
  - Base 검증 후 발주서 고유 검증 (수량 > 0, 공급자 필수)

**Blog Base:**
- `scripts/base/blog_article.js` — 블로그 글 Base
  - `before_publish`: 카테고리 필수, 태그 형식 검증
  - `after_publish`: 스러그(Slug) 생성

**Blog Extension:**
- `scripts/extensions/blog_tech.js` — 기술 블로그
  - Base 검증 후 코딩 언어 태그 추가

**Shop Base:**
- `scripts/base/shop_product.js` — 상품 Base
  - `before_list`: 재고 검증
  - `before_order`: 가격 검증 (rust_calculate_base_tax 사용)

**Shop Extension:**
- `scripts/extensions/shop_electronics.js` — 전자제품
  - Base 검증 후 보증 기간 필드 추가

---

## 2. SDK 바인딩

`SandboxedContext` 생성 시 SDK 함수를 globals에 바인딩:

```rust
// src/engine/runtime.rs (SandboxedContext::new)
// ... 기존 바인딩 ...
// SDK 바인딩
let globals = ctx.globals();
globals.set("rust_calculate_base_tax", rquickjs::function(ctx.clone(), sdk::math::rust_calculate_base_tax))?;
globals.set("rust_calculate_discount", rquickjs::function(ctx.clone(), sdk::math::rust_calculate_discount))?;
globals.set("rust_hash_token", rquickjs::function(ctx.clone(), sdk::utils::rust_hash_token))?;
globals.set("rust_validate_uuid", rquickjs::function(ctx.clone(), sdk::utils::rust_validate_uuid))?;
```

---

## 3. 산출물

| 파일 | 내용 |
|------|------|
| `src/sdk/mod.rs` | SDK 모듈 |
| `src/sdk/math.rs` | 계산 SDK (rust_calculate_base_tax, rust_calculate_discount) |
| `src/sdk/utils.rs` | 유틸 SDK (rust_hash_token, rust_validate_uuid) |
| `src/engine/runtime.rs` | SDK 바인딩 추가 |
| `scripts/base/erp_workflow.js` | ERP Base |
| `scripts/extensions/erp_purchase_order.js` | ERP 발주서 |
| `scripts/base/blog_article.js` | Blog Base |
| `scripts/extensions/blog_tech.js` | 기술 블로그 |
| `scripts/base/shop_product.js` | Shop Base (SDK 호출) |
| `scripts/extensions/shop_electronics.js` | 전자제품 |
| `tests/phase11_sdk_domain.rs` | SDK + 도메인 모듈 테스트 |

---

## 4. 주의사항

1. **SDK 함수는 panic 금지**: `Result<T, rquickjs::Error>` 반환
2. **JS 스크립트는 `var api` 선언 금지**: `api` 인자 사용
3. **Base→Extension 바인딩**: `Base.<hook>`으로 프로토타입 체이닝
4. **에러 코드 6자리**: `SJ1201`, `BJ1201`, `EJ1201` 등
