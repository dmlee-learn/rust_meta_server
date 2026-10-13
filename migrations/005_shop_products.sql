-- Phase 38: Shop 확장 — 상품(shop_products) + 카트(shop_carts)
-- 스키마: 상품 카탈로그와 쇼핑 카트

CREATE TABLE IF NOT EXISTS shop_products (
    id BIGSERIAL PRIMARY KEY,
    tenant_id TEXT NOT NULL DEFAULT 'default',
    name TEXT NOT NULL,
    price NUMERIC(12,2) NOT NULL DEFAULT 0,
    stock INTEGER NOT NULL DEFAULT 0,
    category TEXT NOT NULL DEFAULT '',
    slug TEXT NOT NULL DEFAULT '',
    extra TEXT NOT NULL DEFAULT 'null',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_shop_products_tenant ON shop_products(tenant_id);
CREATE INDEX IF NOT EXISTS idx_shop_products_slug ON shop_products(slug);

CREATE TABLE IF NOT EXISTS shop_carts (
    id BIGSERIAL PRIMARY KEY,
    tenant_id TEXT NOT NULL DEFAULT 'default',
    customer TEXT NOT NULL,
    product_id BIGINT NOT NULL REFERENCES shop_products(id) ON DELETE CASCADE,
    quantity INTEGER NOT NULL DEFAULT 1,
    extra TEXT NOT NULL DEFAULT 'null',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_shop_carts_tenant ON shop_carts(tenant_id);
CREATE INDEX IF NOT EXISTS idx_shop_carts_customer ON shop_carts(customer);
