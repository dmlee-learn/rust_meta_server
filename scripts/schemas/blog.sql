-- ============================================================
-- 블로그(BLOG) 도메인 테이블 통합 스키마
-- 포함 테이블: blog_articles, blog_comments
-- 생성: Phase 41/42 통합
-- ============================================================

-- 1. blog_articles: 블로그 글
CREATE TABLE IF NOT EXISTS blog_articles (
    id                BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    title             TEXT NOT NULL,
    body              TEXT NOT NULL DEFAULT '',
    category          TEXT NOT NULL,
    slug              TEXT NOT NULL DEFAULT '',
    extra             TEXT NOT NULL DEFAULT 'null',
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Phase 37: Multi-tenant
    tenant_id         TEXT NOT NULL DEFAULT 'default',
    -- Phase 41: 템플릿 바인딩
    template_id       TEXT NOT NULL DEFAULT '',
    template_version  BIGINT NOT NULL DEFAULT 0,
    sync_policy       TEXT NOT NULL DEFAULT 'frozen'
);

CREATE INDEX IF NOT EXISTS idx_blog_category ON blog_articles(category);
CREATE INDEX IF NOT EXISTS idx_blog_articles_tenant ON blog_articles(tenant_id);
CREATE INDEX IF NOT EXISTS idx_blog_articles_template ON blog_articles(template_id);

-- 2. blog_comments: 블로그 댓글
CREATE TABLE IF NOT EXISTS blog_comments (
    id          BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    article_id  BIGINT NOT NULL REFERENCES blog_articles(id) ON DELETE CASCADE,
    author      TEXT NOT NULL,
    body        TEXT NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    tenant_id   TEXT NOT NULL DEFAULT 'default'
);

CREATE INDEX IF NOT EXISTS idx_blog_comments_article ON blog_comments(article_id);
CREATE INDEX IF NOT EXISTS idx_blog_comments_tenant ON blog_comments(tenant_id);
