# HANDOVER: Phase 36 — 데이터 스키마 확장

> 이 문서는 Phase 36을 완료하고 단절했을 때, 다음 개발자(또는 다음 세션의 AI 에이전트)가
> 이어서 작업을 진행할 수 있도록 작성된 핸드오버 문서입니다.

---

## 1. Phase 36 완료 내용

### 1.1 Migration (`migrations/003_tags_comments.sql`)
- `board_tags`: (id, board_id, post_id, tag, created_at)
- `blog_comments`: (id, article_id, author, body, parent_id, created_at)
- 인덱스: `idx_board_tags_post`, `idx_blog_comments_article`

### 1.2 모델 + DataStore (`src/meta/store.rs`)
- `BoardTag`, `BlogComment` 모델
- DataStore trait 4 methods:
  - `create_board_tag(&BoardTag) -> i64`
  - `list_board_tags_by_post(post_id) -> Vec<BoardTag>`
  - `create_blog_comment(&BlogComment) -> i64`
  - `list_blog_comments_by_article(article_id) -> Vec<BlogComment>`
- MemoryStore + PostgresStore 구현

### 1.3 API (`src/http.rs`)
- `POST /api/tags` (body: `{board_id, post_id, tag}`)
- `GET /api/tags?post_id=1`
- `POST /api/blog/comments` (body: `{article_id, author, body, parent_id?}`)
- `GET /api/blog/comments?article_id=1`
- `AppState::first_store()`: board 우선, 미르면 domains[0] store 접근
- `AppState::query_param_i64(req, key)`: query string 파싱

### 1.4 테스트
- `board_tag_crud`, `blog_comment_crud` (MemoryStore)
- 전체 106개 통과

### 1.5 Docker 실 검증
- migration: board_tags + blog_comments 테이블 생성
- 태그/댓글 CRUD API PostgreSQL에서 동작 확인

---

## 2. 주의사항

### 2.1 DataStore trait 확장
- trait에 method 추가 시 **모든 구현체**에 구현 필요
- MemoryStore, PostgresStore, FailingStore (test)
- 새 스키마 추가 시 `FailingStore`도 갱신 잊지 말 것

### 2.2 MemoryStore 위치
- `meta_server::meta::store::MemoryStore` (meta::memory 아님)

### 2.3 Store 접근
- `first_store()`: board가 있으면 board.store, 없으면 domains[0].store
- 모든 도메인은 동일한 store를 공유 (단일 DB)

### 2.4 Query 파싱
- `query_param_i64`는 수동 파싱 (시리즈 의존성 미사용)
- `?post_id=1` 형식만 지원

---

## 3. 다음 단계 (Phase 37)

### 3.1 Multi-tenant
- 기업별 DB 격리
- tenant_id 기반 권한

### 3.2 Shop 확장
- 상품 카트/결제
- 스키마: shop_products, shop_carts

---

## 4. 빌드 및 검증

```bash
# 전체 테스트 (106개)
cargo test

# Docker build + verify
docker build -t meta_server:latest .
docker compose up -d --force-recreate postgres meta_server

# migration 확인
docker compose exec postgres psql -U meta -d meta -c "\dt" | grep -E "board_tags|blog_comments"

# 태그 CRUD
curl -X POST http://localhost:3000/api/tags -H "Content-Type: application/json" -d '{"board_id":"board","post_id":1,"tag":"중요"}'
curl "http://localhost:3000/api/tags?post_id=1"
```
