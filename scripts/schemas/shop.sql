-- ============================================================
-- 쇼핑몰(SHOP) 도메인 테이블 통합 스키마
-- 포함 테이블: shop_products, shop_carts, shop_orders
-- 생성: Phase 41/42 통합
-- ============================================================

-- 1. shop_products: 상품 카탈로그
CREATE TABLE IF NOT EXISTS shop_products (
    id                BIGSERIAL PRIMARY KEY,
    tenant_id         TEXT NOT NULL DEFAULT 'default',
    name              TEXT NOT NULL,
    price             NUMERIC(12,2) NOT NULL DEFAULT 0,
    stock             INTEGER NOT NULL DEFAULT 0,
    category          TEXT NOT NULL DEFAULT '',
    slug              TEXT NOT NULL DEFAULT '',
    extra             TEXT NOT NULL DEFAULT 'null',
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Phase 41: 템플릿 바인딩
    template_id       TEXT NOT NULL DEFAULT '',
    template_version  BIGINT NOT NULL DEFAULT 0,
    sync_policy       TEXT NOT NULL DEFAULT 'frozen'
);

CREATE INDEX IF NOT EXISTS idx_shop_products_tenant ON shop_products(tenant_id);
CREATE INDEX IF NOT EXISTS idx_shop_products_slug ON shop_products(slug);
CREATE INDEX IF NOT EXISTS idx_shop_products_template ON shop_products(template_id);

-- 2. shop_carts: 쇼핑 카트
CREATE TABLE IF NOT EXISTS shop_carts (
    id          BIGSERIAL PRIMARY KEY,
    tenant_id   TEXT NOT NULL DEFAULT 'default',
    customer    TEXT NOT NULL,
    product_id  BIGINT NOT NULL REFERENCES shop_products(id) ON DELETE CASCADE,
    quantity    INTEGER NOT NULL DEFAULT 1,
    extra       TEXT NOT NULL DEFAULT 'null',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_shop_carts_tenant ON shop_carts(tenant_id);
CREATE INDEX IF NOT EXISTS idx_shop_carts_customer ON shop_carts(customer);

-- 3. shop_orders: 상품 주문
CREATE TABLE IF NOT EXISTS shop_orders (
    id              BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    product_id      TEXT NOT NULL,
    price           DOUBLE PRECISION NOT NULL CHECK (price >= 0),
    quantity        INTEGER NOT NULL CHECK (quantity > 0),
    status          TEXT NOT NULL DEFAULT 'pending',
    extra           TEXT NOT NULL DEFAULT 'null',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    tenant_id       TEXT NOT NULL DEFAULT 'default'
);

CREATE INDEX IF NOT EXISTS idx_shop_product ON shop_orders(product_id);
CREATE INDEX IF NOT EXISTS idx_shop_orders_tenant ON shop_orders(tenant_id);
