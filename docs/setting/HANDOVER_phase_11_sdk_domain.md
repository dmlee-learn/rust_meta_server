# HANDOVER: Phase 11 — SDK 구현 + 추가 도메인 모듈

> 이 문서는 Phase 11을 완료하고 단절했을 때, 다음 개발자(또는 다음 세션의 AI 에이전트)가
> 이어서 작업을 진행할 수 있도록 작성된 핸드오버 문서입니다.

---

## 1. Phase 11 완료 내용

### 1.1 SDK 구현

`src/sdk/` 디렉토리 신규 생성:

| 파일 | 내용 |
|------|------|
| `src/sdk/mod.rs` | SDK 모듈 선언 |
| `src/sdk/math.rs` | 계산 계열: `rust_calculate_base_tax()`, `rust_calculate_discount()` |
| `src/sdk/utils.rs` | 유틸 계열: `rust_hash_token()`, `rust_validate_uuid()` |

**SDK 함수 시그니처:**

| 함수 | 시그니처 | 설명 |
|------|----------|------|
| `rust_calculate_base_tax` | `fn(amount: f64, rate: f64) -> f64` | 원가 + 부가세 (무효 → NaN) |
| `rust_calculate_discount` | `fn(price: f64, pct: f64) -> f64` | 할인율 적용 (무효 → NaN) |
| `rust_hash_token` | `fn(token: String, secret: String) -> String` | FNV-1a 해시 (빈 입력 → "") |
| `rust_validate_uuid` | `fn(uuid: String) -> bool` | UUID v4 형식 검증 |

### 1.2 SDK 바인딩

`src/engine/runtime.rs`의 `create_sandbox()`에서 `rquickjs::Function::new()`로 4개 함수를 바인딩:

```rust
ctx.with(|ctx| {
    let globals = ctx.globals();
    globals.set("rust_calculate_base_tax", rquickjs::Function::new(ctx.clone(),
        |_ctx: rquickjs::Ctx, a: f64, r: f64| -> rquickjs::Result<f64> {
            Ok(crate::sdk::math::rust_calculate_base_tax(a, r))
        }))?;
    // ... 나머지 3개 동일 패턴
    Ok::<(), rquickjs::Error>(())
}).map_err(|_| InfraError::Cr002)?;
```

### 1.3 추가 도메인 모듈 (테스트 시나리오)

Phase 11은 SDK + 모듈 구조를 검증하기 위한 테스트 시나리오를 포함합니다:

| 도메인 | Base | Extension |
|--------|------|-----------|
| ERP | `before_process` (워크플로우 ID 검증) | `erp_purchase_order` (수량 > 0, 삭제 불가) |
| Blog | `before_publish` (카테고리/제목 필수), `after_publish` (슬러그 생성) | `blog_tech` (languages 필드 추가) |
| Shop | `before_order` (가격 검증 + `rust_calculate_base_tax` 호출) | `shop_electronics` (보증 기간 필드) |

### 1.4 전체 테스트: **73개 전부 통과**

| 테스트 파일 | 개수 |
|-------------|------|
| `src/sdk` (math + utils) | 12 |
| `tests/phase11_sdk_domain.rs` | 6 |
| 기존 (Phase 1~10) | 55 |
| **합계** | **73** |

---

## 2. 주의사항

### 2.1 rquickjs 0.14 함수 바인딩 API

```rust
// rquickjs 0.14에서 함수 바인딩: Function::new
rquickjs::Function::new(ctx.clone(), |ctx: rquickjs::Ctx, arg1: T1, arg2: T2| -> rquickjs::Result<T> {
    Ok(result)
})?;
```

- `rquickjs::function`은 **모듈**이므로 직접 호출 불가
- `Function::new`의 첫 번째 인자: `Ctx<'_>` (JS 컨텍스트)
- 클로저가 `rquickjs::Result<T>`를 반환해야 함

### 2.2 SDK 에러 처리

- 컨텍스트 없이 동작하는 순수 함수이므로, 에러 시 `NaN`(f64) 또는 `""`(String) 반환
- JS에서 `isNaN()`, `=== ""`로 무효 확인
- panic하지 않음 (AGENTS.md 규정)

### 2.3 UUID v4 검증

- 3번째 그룹(0-indexed: 2) 첫 글자 = `4` (버전 숫자)
- 4번째 그룹(0-indexed: 3) 첫 글자 = `8`/`9`/`a`/`b` (변수)
- 총 36자: `8-4-4-4-12` 16진수

---

## 3. 다음 단계 (Phase 12)

### 3.1 운영 배포 패키지
- Dockerfile 작성
- `cargo build --release`
- 실 환경 배포 가이드

### 3.2 모듈 로드 파이프라인 확장
- `ModuleLoader`에 ERP/Blog/Shop 모듈 지원
- `BoardPipeline` → `DomainPipeline` 추상화

### 3.3 데이터 스키마 확장
- ERP, Blog, Shop 테이블 추가
- `migrations/002_domain_tables.sql`

---

## 4. 빌드 및 테스트

```bash
# 전체 테스트 (73개)
cargo test

# Phase 11 테스트만
cargo test --test phase11_sdk_domain
```
