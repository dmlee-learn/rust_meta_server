# Phase 37: Multi-Tenant (tenant_id 격리)

## Goal
멀티테넌트 데이터 격리 — `tenant_id` 컬럼 추가, scoped API, API 레벨 tenant 분리.

## Decisions
- **D37-1**: **테넌트 식별** — `?tenant=` query 파라미터 또는 `X-Tenant-Id` 헤더. 둘 다 없으면 `"default"`.
- **D37-2**: **격리 레벨** — Row-level (tenant_id 컬럼 + WHERE filter). Row-Level Security(RLS) 정책은 추후 고려 (DBA 권한 필요).
- **D37-3**: **대상 테이블** — `board_posts`, `erp_purchase_orders`, `blog_articles`, `shop_orders`, `board_tags`, `blog_comments` (공개 도메인 전부).
- **D37-4**: **scoped 메서드** — 기존 `list_*`/`save_*` 시그니처 유지(호환성). 새 `*_for_tenant` 메서드 추가 (4개).
- **D37-5**: **기본값** — `tenant_id TEXT NOT NULL DEFAULT 'default'`. 기존 데이터는 `"default"` 테넌트에 귀속.
- **D37-6**: **관리자 API** — `X-Admin-Token` 경유 `/api/admin/*`는 tenant 격리 우회 (운영자 시점, 전 테넌트 가시).

## Files
- `migrations/004_tenant.sql` — 6 테이블에 `tenant_id` 컬럼 + 인덱스 추가
- `src/meta/store.rs` —
  - `tenant_id` 필드 추가 (BoardPost, ErpPurchaseOrder, BlogArticle, ShopOrder, BoardTag, BlogComment)
  - `default_tenant()` 헬퍼 (serde default)
  - `DataStore` trait에 scoped 메서드 4개:
    - `list_board_posts_for_tenant`
    - `list_blog_articles_for_tenant`
    - `list_shop_orders_for_tenant`
    - `list_erp_orders_for_tenant`
  - `PostgresStore` + `MemoryStore` 구현
  - INSERT/SELECT 쿼리에 `tenant_id` 포함
- `src/http.rs` —
  - `GET /api/tenant/posts?tenant=acme` 엔드포인트 (tenant별 게시판 글 목록)
  - `X-Tenant-Id` 헤더 + `?tenant=` query 둘 다 지원
- `src/domain.rs`, `src/pipeline.rs` — entity 생성 시 `tenant_id: "default"` 기본값
- `tests/phase29_integration.rs` — `tenant_isolation_scoped_list` (D37-1)
- `tests/phase12_domain.rs` — entity 생성 시 `tenant_id` 필드 추가

## API

### `GET /api/tenant/posts?tenant={id}`
- tenant별 게시판 글 목록
- 응답: `{ "tenant": "acme", "count": 1, "posts": [...] }`
- `?tenant=` 없으면 `X-Tenant-Id` 헤더, 없으면 `"default"`

## Verification
- **빌드**: OK
- **테스트**: 107개 통과 (기존 106 + 신규 1)
- **Docker**:
  - `004_tenant.sql` 마이그레이션 적용 확인 (`tenant_id` 컬럼 + 인덱스)
  - `?tenant=default` → 1 post 반환
  - `?tenant=acme` → 0 post 반환 (격리 정상)

## Errors Encountered
1. **`ALTER TABLE IF NOT EXISTS` 무효**: PG는 테이블명에 `IF NOT EXISTS` 불가. `ADD COLUMN IF NOT EXISTS`만 사용.
2. **`&[tenant_id]` 타입 불일치**: `tenant_id: &str`일 때 `&[tenant_id]`는 `&[&str]`로 ToSql 미충족. `&[&tenant_id]` (double ref)로 해결.

## Next Steps
- Phase 38: Shop 확장 (상품/카트 스키마)
- 추후: DBA 권한으로 PostgreSQL RLS 정책 적용 (`ALTER TABLE ... ENABLE ROW LEVEL SECURITY`)
- 추후: Admin API에도 tenant filter 지원 (테넌트별 대시보드)
