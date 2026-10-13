-- Phase 36: 게시판 태그 + 블로그 댓글
-- 기존: 001_init.sql, 002_domain_tables.sql
-- 이 파일: board_tags, blog_comments 테이블 추가

-- 게시판 태그
CREATE TABLE IF NOT EXISTS board_tags (
    id          BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    board_id    TEXT NOT NULL,
    post_id     BIGINT NOT NULL,
    tag         TEXT NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- 블로그 댓글
CREATE TABLE IF NOT EXISTS blog_comments (
    id          BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    article_id  BIGINT NOT NULL,
    author      TEXT NOT NULL,
    body        TEXT NOT NULL DEFAULT '',
    parent_id   BIGINT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- 인덱스
CREATE INDEX IF NOT EXISTS idx_board_tags_post ON board_tags(post_id);
CREATE INDEX IF NOT EXISTS idx_blog_comments_article ON blog_comments(article_id);
