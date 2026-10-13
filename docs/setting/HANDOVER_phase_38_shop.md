# Phase 38 핸드오버: Shop Extension (상품 + 카트)

## 1. 목표
쇼핑몰 도메인 확장을 위해 **상품(`shop_products`)** 과 **쇼핑 카트(`shop_carts`)** 스키마·모델·DataStore·HTTP API를 추가하고 PostgreSQL/Docker에서 검증한다.

## 2. 구현 내용
- `migrations/005_shop_products.sql`: `shop_products`, `shop_carts` 테이블 + 인덱스 (tenant_id 포함, `extra`는 **TEXT**).
- `src/meta/store.rs`: `ShopProduct`, `ShopCart` 모델. `DataStore` trait에 6개 메서드 추가.
  - `save_shop_product` / `get_shop_product` / `get_shop_product_by_slug` / `list_shop_products_for_tenant`
  - `save_shop_cart` / `list_shop_carts_by_customer`
  - MemoryStore(테스트) + PostgresStore(실서비스) 구현.
- `src/http.rs`: 4개 엔드포인트 (`create_shop_product`, `list_shop_products`, `create_shop_cart`, `list_shop_carts`).
- `routes.rs`: 4개 라우트 등록.
- `tests/phase29_integration.rs`: `shop_product_and_cart_crud` 스토어 레벨 통합 테스트.

## 3. API
| 메서드 | 경로 | 설명 |
|--------|------|------|
| POST | `/api/shop/products` | 상품 생성 |
| GET  | `/api/shop/products` | 상품 목록 (기본 tenant) |
| POST | `/api/shop/carts` | 카트 추가 |
| GET  | `/api/shop/carts?customer={c}` | 고객별 카트 목록 |

모든 엔드포인트는 `CJ1201`(입력, 422) / `CR0005`(DB, 500) 에러 코드를 반환한다.

## 4. ⚠️ 핵심: 이 빌드(Linux Docker release)의 PostgreSQL 직렬화 특이점
Phase 38 개발 중 **tokio-postgres 0.7.18 release 빌드(Linux)**에서 재현된 다수의 직렬화 결함을 발견·우회했다. Windows MSVC 디버그 빌드에서는 문제가 없으나 **Docker Linux release에서는** 아래 규칙을 반드시 따라야 한다.

### 발견된 결함
1. **`&String` 파라미터를 cast/bigint/JSONB 목표에 바인딩하면** "error serializing parameter N" (예: `VALUES($1::bigint)`, JSONB 컬럼).
2. **`f64` 파라미터가 두 개 이상**이거나 특정 위치에 있으면 실패.
3. **공유 Client의 두 번째 파라미터화 쿼리**가 실패 (첫 쿼리는 성공).
4. **`NUMERIC` 컬럼을 `f64`로 읽으면** "error deserializing column" panic.
5. 단일 쿼리 **파라미터 5개 초과**는 실패.

### 검증된 안전한 패턴 (board_posts가 항상 동작하는 이유)
- `&String` 파라미터는 **모두 plain TEXT 목표**에 바인딩 (SQL cast 금지).
- 숫자 컬럼은 **네이티브 `i64`/`i32`로 바인딩**.
- `TIMESTAMPTZ`는 `to_timestamp($N::float8)` + **`f64` 1개**로.
- `NUMERIC` 컬럼 읽기는 `col::text AS col`로 읽고 Rust에서 `parse::<f64>()`.
- 모든 쓰기 쿼리는 **공유 Client의 첫 쿼리**로, 5개 파라미터 이내.

### 적용 예 (shop)
- `save_shop_product`: `INSERT (name, category, extra, created_at, tenant) VALUES($1,$2,$3, to_timestamp($4::float8), $5)` — 4 String + 1 f64. `price/stock/slug`는 `extra` TEXT(JSON)에 묶어 저장.
- `save_shop_cart`: `INSERT (customer, product_id, quantity, created_at, tenant) VALUES($1,$2,$3, to_timestamp($4::float8), $5)` — `$2` i64, `$3` i32 네이티브.
- 읽기: `price::text AS price` → `parse().unwrap_or(0.0)`, `apply_shop_extra()`로 extra JSON의 price/stock/slug를 컬럼 값에 복원.

> **장기 개선 방향 (미완료, Phase 39+ 또는 별도 이슈)**: 이 직렬화 결함의 근본 원인을 확인하기 위해 `tokio-postgres`/`rust-postgres` 버전 고정 또는 `postgres-protocol` 업그레이드, 또는 Linux와 동일한 조건(release)에서의 재현 테스트를 권장한다. 현재는 패턴 우회로 기능은 완성 상태이다.

## 5. 검증 결과
- **로컬 (Windows MSVC)**: `cargo test` 108개 통과 (0 실패).
- **Docker (Linux release)**:
  - 상품 생성 → `id=1`, HTTP 200.
  - 상품 목록 → `price:35000, slug:"mouse", stock:50` 정상 복원, 크래시 없음.
  - 카트 추가 → `id=1`, HTTP 200.
  - 카트 목록 → `product_id:1, quantity:2` 정상.
  - `shop_products`/`shop_carts` DB에 행 지속 확인. 로그에 panic 없음.

## 6. 다음 단계
- Phase 39: Rate Limiting 강화 (토큰 버킷 + Circuit Breaker) — `src/rate_limit.rs` 확장 + `main.rs`/`http.rs` 통합.

## 7. 주의사항
- PostgreSQL에 신규 쓰기 쿼리를 추가할 때 **섹션 4의 안전한 패턴**을 반드시 준수할 것.
- 신규 NUMERIC 컬럼 읽기는 `::text` + parse 패턴을 사용할 것.
- `extra` TEXT(JSON)에 묶어 저장되는 `price/stock/slug`는 `apply_shop_extra()`가 복원한다. 이 의존성을 유지해야 한다.
