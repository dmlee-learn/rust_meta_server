-- Phase 37: Multi-tenant — tenant_id 컬럼 추가 (Row-level 격리)
-- 대상: 공개 도메인 테이블 전부
-- PG 문법: ALTER TABLE에 IF NOT EXISTS 불가 → ADD COLUMN IF NOT EXISTS만 사용

ALTER TABLE board_posts
    ADD COLUMN IF NOT EXISTS tenant_id TEXT NOT NULL DEFAULT 'default';

ALTER TABLE erp_purchase_orders
    ADD COLUMN IF NOT EXISTS tenant_id TEXT NOT NULL DEFAULT 'default';

ALTER TABLE blog_articles
    ADD COLUMN IF NOT EXISTS tenant_id TEXT NOT NULL DEFAULT 'default';

ALTER TABLE shop_orders
    ADD COLUMN IF NOT EXISTS tenant_id TEXT NOT NULL DEFAULT 'default';

ALTER TABLE board_tags
    ADD COLUMN IF NOT EXISTS tenant_id TEXT NOT NULL DEFAULT 'default';

ALTER TABLE blog_comments
    ADD COLUMN IF NOT EXISTS tenant_id TEXT NOT NULL DEFAULT 'default';

-- 인덱스 (tenant별 조회 최적화)
CREATE INDEX IF NOT EXISTS idx_board_posts_tenant ON board_posts(tenant_id);
CREATE INDEX IF NOT EXISTS idx_erp_po_tenant ON erp_purchase_orders(tenant_id);
CREATE INDEX IF NOT EXISTS idx_blog_articles_tenant ON blog_articles(tenant_id);
CREATE INDEX IF NOT EXISTS idx_shop_orders_tenant ON shop_orders(tenant_id);
CREATE INDEX IF NOT EXISTS idx_board_tags_tenant ON board_tags(tenant_id);
CREATE INDEX IF NOT EXISTS idx_blog_comments_tenant ON blog_comments(tenant_id);
