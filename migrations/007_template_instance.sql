-- ============================================================
-- Phase 41 Step 2: 템플릿 인스턴스 확장 (template_version / sync_policy)
-- 단1(migration 006)에서 template_id 컬럼만 추가됨.
-- 단2에서 인스턴스가 어떤 템플릿 버전에 바인딩되었는지 + 개선 전파 정책을 기록.
--  - template_version: 인스턴스가 생성 시 스냅샷한 템플릿 버전 (frozen 시 불변).
--  - sync_policy: 'frozen'(기본값, 템플릿 변경 무시) | 'live'(propagate 시 payload 재적용).
-- 멱등: ADD COLUMN IF NOT EXISTS.
-- ============================================================

-- 게시판 글
ALTER TABLE board_posts ADD COLUMN IF NOT EXISTS template_version BIGINT NOT NULL DEFAULT 0;
ALTER TABLE board_posts ADD COLUMN IF NOT EXISTS sync_policy      TEXT   NOT NULL DEFAULT 'frozen';

-- 블로그 글
ALTER TABLE blog_articles ADD COLUMN IF NOT EXISTS template_version BIGINT NOT NULL DEFAULT 0;
ALTER TABLE blog_articles ADD COLUMN IF NOT EXISTS sync_policy      TEXT   NOT NULL DEFAULT 'frozen';

-- 쇼핑몰 상품
ALTER TABLE shop_products ADD COLUMN IF NOT EXISTS template_version BIGINT NOT NULL DEFAULT 0;
ALTER TABLE shop_products ADD COLUMN IF NOT EXISTS sync_policy      TEXT   NOT NULL DEFAULT 'frozen';

-- propagate 대상 검색용 인덱스 (sync_policy='live' 인스턴스).
CREATE INDEX IF NOT EXISTS idx_board_posts_sync_policy ON board_posts(sync_policy);
CREATE INDEX IF NOT EXISTS idx_blog_articles_sync_policy ON blog_articles(sync_policy);
CREATE INDEX IF NOT EXISTS idx_shop_products_sync_policy ON shop_products(sync_policy);
