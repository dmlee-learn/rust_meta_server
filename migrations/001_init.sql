-- MetaServer PostgreSQL 스키마 (Phase 4)
-- 실행: psql -U <user> -d <db> -f migrations/001_init.sql

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

-- 2. board_posts: 게시판 글 (Phase 3 시나리오)
CREATE TABLE IF NOT EXISTS board_posts (
    id          BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    title       TEXT NOT NULL,
    body        TEXT NOT NULL DEFAULT '',
    extra       TEXT NOT NULL DEFAULT 'null',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
