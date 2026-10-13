-- ============================================================
-- 게시판(BOARD) 도메인 테이블 통합 스키마
-- 포함 테이블: board_posts, board_tags
-- 생성: Phase 41/42 통합
-- ============================================================

-- 1. board_posts: 게시판 글
CREATE TABLE IF NOT EXISTS board_posts (
    id                BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    title             TEXT NOT NULL,
    body              TEXT NOT NULL DEFAULT '',
    extra             TEXT NOT NULL DEFAULT 'null',
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Phase 37: Multi-tenant
    tenant_id         TEXT NOT NULL DEFAULT 'default',
    -- Phase 41: 템플릿 바인딩
    template_id       TEXT NOT NULL DEFAULT '',
    template_version  BIGINT NOT NULL DEFAULT 0,
    sync_policy       TEXT NOT NULL DEFAULT 'frozen'
);

CREATE INDEX IF NOT EXISTS idx_board_posts_tenant ON board_posts(tenant_id);
CREATE INDEX IF NOT EXISTS idx_board_posts_template ON board_posts(template_id);

-- 2. board_tags: 게시판 태그
CREATE TABLE IF NOT EXISTS board_tags (
    id          BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    post_id     BIGINT NOT NULL REFERENCES board_posts(id) ON DELETE CASCADE,
    tag         TEXT NOT NULL,
    tenant_id   TEXT NOT NULL DEFAULT 'default'
);

CREATE INDEX IF NOT EXISTS idx_board_tags_post ON board_tags(post_id);
CREATE INDEX IF NOT EXISTS idx_board_tags_tenant ON board_tags(tenant_id);
