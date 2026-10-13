-- ============================================================
-- ERP 도메인 테이블 통합 스키마
-- 포함 테이블: erp_purchase_orders
-- 생성: Phase 41/42 통합
-- ============================================================

CREATE TABLE IF NOT EXISTS erp_purchase_orders (
    id          BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    workflow_id TEXT NOT NULL,
    quantity    INTEGER NOT NULL CHECK (quantity > 0),
    supplier    TEXT NOT NULL,
    status      TEXT NOT NULL DEFAULT 'pending',
    extra       TEXT NOT NULL DEFAULT 'null',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    tenant_id   TEXT NOT NULL DEFAULT 'default'
);

CREATE INDEX IF NOT EXISTS idx_erp_po_workflow ON erp_purchase_orders(workflow_id);
CREATE INDEX IF NOT EXISTS idx_erp_po_tenant ON erp_purchase_orders(tenant_id);
