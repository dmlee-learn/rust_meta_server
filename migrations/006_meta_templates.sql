-- Phase 41: 계층형 템플릿 시스템
-- 템플릿은 트리 노드(meta_templates)로, parent_id로 계층(page→menu→body)을 이룸.
-- 기존 도메인 테이블에 template_id로 출처 기록.

-- 1. meta_templates: 계층형 템플릿 노드
CREATE TABLE IF NOT EXISTS meta_templates (
    template_id   TEXT NOT NULL,
    tenant_id     TEXT NOT NULL DEFAULT 'default',
    parent_id     TEXT NOT NULL DEFAULT '',   -- '' = 루트(page/site). 비'' = 하위 메뉴/본문
    name          TEXT NOT NULL DEFAULT '',
    ttype         TEXT NOT NULL DEFAULT 'body',  -- 'page'|'menu'|'body'|'field'|'site'
    domain        TEXT NOT NULL DEFAULT '',      -- 'board'|'blog'|'shop'|'homepage'|''
    payload       TEXT NOT NULL DEFAULT '{}',   -- JSON: 필드 스키마/레이아웃/기본값
    source        TEXT NOT NULL DEFAULT 'db',   -- 'db' | 'file'
    file_path     TEXT NOT NULL DEFAULT '',
    version       BIGINT NOT NULL DEFAULT 1,
    enabled       BOOLEAN NOT NULL DEFAULT TRUE,
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (template_id, tenant_id, version)
);

CREATE INDEX IF NOT EXISTS idx_meta_templates_active ON meta_templates(tenant_id, parent_id, enabled);
CREATE INDEX IF NOT EXISTS idx_meta_templates_domain ON meta_templates(tenant_id, domain);

-- 2. 기존 도메인 테이블에 템플릿 출처 기록
ALTER TABLE board_posts ADD COLUMN IF NOT EXISTS template_id TEXT;
ALTER TABLE blog_articles ADD COLUMN IF NOT EXISTS template_id TEXT;
ALTER TABLE shop_products ADD COLUMN IF NOT EXISTS template_id TEXT;
