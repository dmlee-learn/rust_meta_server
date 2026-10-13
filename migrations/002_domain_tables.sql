-- Phase 12: ERP/Blog/Shop 도메인 테이블
-- 기존: migrations/001_init.sql (meta_modules, board_posts)
-- 이 파일: ERP 발주서, Blog 글, Shop 주문 테이블 추가

-- ERP: 발주서
CREATE TABLE IF NOT EXISTS erp_purchase_orders (
    id          BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    workflow_id TEXT NOT NULL,
    quantity    INTEGER NOT NULL CHECK (quantity > 0),
    supplier    TEXT NOT NULL,
    status      TEXT NOT NULL DEFAULT 'pending',
    extra       TEXT NOT NULL DEFAULT 'null',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Blog: 글
CREATE TABLE IF NOT EXISTS blog_articles (
    id          BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    title       TEXT NOT NULL,
    body        TEXT NOT NULL DEFAULT '',
    category    TEXT NOT NULL,
    slug        TEXT NOT NULL DEFAULT '',
    extra       TEXT NOT NULL DEFAULT 'null',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Shop: 상품 주문
CREATE TABLE IF NOT EXISTS shop_orders (
    id          BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    product_id  TEXT NOT NULL,
    price       DOUBLE PRECISION NOT NULL CHECK (price >= 0),
    quantity    INTEGER NOT NULL CHECK (quantity > 0),
    status      TEXT NOT NULL DEFAULT 'pending',
    extra       TEXT NOT NULL DEFAULT 'null',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- 인덱스
CREATE INDEX IF NOT EXISTS idx_erp_po_workflow ON erp_purchase_orders(workflow_id);
CREATE INDEX IF NOT EXISTS idx_blog_category ON blog_articles(category);
CREATE INDEX IF NOT EXISTS idx_shop_product ON shop_orders(product_id);
