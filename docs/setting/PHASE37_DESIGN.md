# Phase 37 설계: Multi-tenant (scoped accessor)

> Shared DB + `tenant_id` 컬럼 + API 레벨 격리. 기존 시그니처 보존 + `_for_tenant` scoped method 추가.

---

## 결정 사항
- **Q1**: O1-1 전체 공개 도메인 격리 (scoped accessor)
- **Q2**: O2-1 `X-Tenant-Id` 헤더 (없으면 `default`)
- **Q3**: O3-1 admin API 우회 (모든 tenant 조회)

---

## 1. 격리 전략

### D37-1: Shared DB, tenant_id column (Row-level)
- 각 도메인 테이블에 `tenant_id TEXT NOT NULL DEFAULT 'default'`
- 요청 시 `X-Tenant-Id` 헤더로 tenant 추출 (없으면 `default`)

### D37-2: scoped accessor (기존 시그니처 보존)
- 기존 `create_X`/`list_X`는 유지 (default tenant)
- 신규 `create_X_for_tenant`/`list_X_for_tenant` 추가
- HTTP 핸들러는 tenant 추출 후 scoped method 호출

---

## 2. 스키마 (004_tenant.sql)

### D37-3: tenant_id 컬럼 + 인덱스
- 대상: `board_posts`, `board_tags`, `blog_articles`, `blog_comments`,
  `erp_purchase_orders`, `shop_orders`
- `ALTER TABLE ... ADD COLUMN IF NOT EXISTS tenant_id TEXT NOT NULL DEFAULT 'default'`
- 인덱스: `idx_<table>_tenant`

---

## 3. 구현 (scoped accessor)

### D37-4: 모델에 tenant_id
- `BoardPost`, `BoardTag`, `BlogArticle`, `BlogComment`,
  `ErpPurchaseOrder`, `ShopOrder`에 `tenant_id: String` (default "default")

### D37-5: DataStore scoped methods (6개)
- `list_board_posts_for_tenant(&self, tenant_id: &str) -> Result<Vec<BoardPost>, ...>`
- `create_board_post_for_tenant(&self, post: &BoardPost, tenant_id: &str) -> Result<i64, ...>`
- `list_blog_articles_for_tenant`, `create_blog_article_for_tenant`
- `list_shop_orders_for_tenant`, `create_shop_order_for_tenant`
- (tags/comments: `list_*_for_tenant` + `create_*_for_tenant`)

### D37-6: HTTP 핸들러
- `tenant_from_request(req) -> String` 헬퍼 (X-Tenant-Id, default "default")
- 신규 create/list 엔드포인트는 tenant 적용
- 기존 엔드포인트: backward-compat (default tenant)
- admin: 우회

---

## 4. 파일 변경

| 파일 | 변경 |
|------|------|
| `migrations/004_tenant.sql` | tenant_id + 인덱스 |
| `src/meta/store.rs` | 모델 tenant_id + scoped methods |
| `src/http.rs` | tenant_from_request + scoped 엔드포인트 |
| `tests/phase29_integration.rs` | tenant 격리 테스트 |

---

## 5. 검증
- 동일 tenant 데이터만 조회
- 다른 tenant 데이터 접근 불가
- 기존 106개 + 신규 통과
