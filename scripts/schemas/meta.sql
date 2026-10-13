-- ============================================================
-- 메타(META) 인프라 테이블 통합 스키마
-- 포함 테이블: meta_modules, meta_templates
-- 생성: Phase 41/42 통합
-- ============================================================

-- 1. meta_modules: 스크립트 메타데이터 (L3)
CREATE TABLE IF NOT EXISTS meta_modules (
    module_id     TEXT PRIMARY KEY,
    kind          TEXT NOT NULL,          -- 'base' | 'extension'
    base_module_id TEXT,                   -- extension만, NULL 가능
    script_source TEXT NOT NULL,
    content_hash  TEXT NOT NULL,
    version       BIGINT NOT NULL DEFAULT 1,
    enabled       BOOLEAN NOT NULL DEFAULT TRUE,
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_meta_modules_kind ON meta_modules(kind);
CREATE INDEX IF NOT EXISTS idx_meta_modules_enabled ON meta_modules(enabled);

-- 2. meta_templates: 계층형 템플릿 노드 (Phase 41)
CREATE TABLE IF NOT EXISTS meta_templates (
    template_id   TEXT NOT NULL,
    tenant_id     TEXT NOT NULL DEFAULT 'default',
    version       INTEGER NOT NULL DEFAULT 1,
    parent_id     TEXT,
    domain        TEXT NOT NULL DEFAULT '',
    ttype         TEXT NOT NULL DEFAULT 'body',
    name          TEXT NOT NULL DEFAULT '',
    source        TEXT NOT NULL DEFAULT 'db',
    file_path     TEXT NOT NULL DEFAULT '',
    payload       TEXT NOT NULL DEFAULT '{}',
    enabled       BOOLEAN NOT NULL DEFAULT TRUE,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (template_id, tenant_id, version)
);

CREATE INDEX IF NOT EXISTS idx_meta_templates_tenant ON meta_templates(tenant_id);
CREATE INDEX IF NOT EXISTS idx_meta_templates_parent ON meta_templates(parent_id);
