# Phase 36 설계: 데이터 스키마 확장

> 게시판 태그 + 블로그 댓글 (기존 패턴 준수: migration + model + DataStore + API).

---

## 1. 신규 엔티티

### D36-1: BoardTag (게시판 태그)
- 테이블: `board_tags`
  - `id BIGINT PK`
  - `board_id TEXT` (연결 게시판)
  - `post_id BIGINT` (연결 글)
  - `tag TEXT`
  - `created_at TIMESTAMPTZ`
- 인덱스: `idx_board_tags_post ON board_tags(post_id)`

### D36-2: BlogComment (블로그 댓글)
- 테이블: `blog_comments`
  - `id BIGINT PK`
  - `article_id BIGINT` (연결 글)
  - `author TEXT`
  - `body TEXT`
  - `parent_id BIGINT NULL` (대댓글)
  - `created_at TIMESTAMPTZ`
- 인덱스: `idx_blog_comments_article ON blog_comments(article_id)`

---

## 2. 파일 변경 (기존 패턴)

| 파일 | 변경 |
|------|------|
| `migrations/003_tags_comments.sql` | 신규 테이블 |
| `src/meta/store.rs` | BoardTag/BlogComment 모델 + DataStore trait methods (4개) |
| `src/meta/memory.rs` | MemoryStore 구현 |
| `src/meta/psql.rs` | PsqlStore 구현 |
| `src/http.rs` | `/tags` (create+list by post), `/blog/comments` (create+list by article) |

### DataStore 신규 methods
- `create_board_tag(&BoardTag) -> i64`
- `list_board_tags_by_post(post_id: i64) -> Vec<BoardTag>`
- `create_blog_comment(&BlogComment) -> i64`
- `list_blog_comments_by_article(article_id: i64) -> Vec<BlogComment>`

---

## 3. API

### D36-3: 태그
- `POST /tags` (body: `{board_id, post_id, tag}`) → create
- `GET /tags?post_id=1` → list by post

### D36-4: 댓글
- `POST /blog/comments` (body: `{article_id, author, body}`) → create
- `GET /blog/comments?article_id=1` → list by article

---

## 4. 검증

- Migration Docker 실 적용 (003)
- 태그/댓글 CRUD API 확인
- 기존 104개 + 신규 테스트 통과
