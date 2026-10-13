# Phase 12 설계: 운영 배포 패키지 + DomainPipeline 추상화

> Dockerfile + release 빌드 + BoardPipeline을 DomainPipeline으로 추상화 +
> ERP/Blog/Shop 데이터 스키마 확장.

---

## 1. 설계 결정

### D1: Dockerfile

**결정: 멀티 스테이지 빌드 (rust:1.79-slim → debian:bookworm-slim)**

```dockerfile
FROM rust:1.79-slim AS builder
WORKDIR /app
COPY . .
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
RUN cargo build --release

FROM debian:bookworm-slim
WORKDIR /app
RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/meta_server /usr/local/bin/
COPY migrations/ /app/migrations/
COPY scripts/ /app/scripts/
ENV PORT=3000
EXPOSE 3000
CMD ["meta_server"]
```

### D2: DomainPipeline 추상화

**결정: `BoardPipeline`의 핵심 로직을 `DomainPipeline`으로 일반화**

```rust
pub struct DomainPipeline {
    pub domain: String,           // "board", "erp", "blog", "shop"
    pub base_module_id: String,   // "base.board", "base.erp" 등
    pub ext_module_id: String,    // "ext.notice", "ext.erp_po" 등
    pub loader: ModuleLoaderRef,
    pub cache: Arc<L1Cache>,
    pub store: Arc<dyn DataStore>,
    pub observability: Arc<Observability>,
}
```

- `handle_create` → `handle_request(payload: serde_json::Value)`으로 일반화
- 훅 이름은 도메인별: `before_create`, `before_process`, `before_publish`, `before_order`
- 응답은 도메인별 `DomainResponse` (id, code, message, data)

### D3: 데이터 스키마 확장

**결정: `migrations/002_domain_tables.sql`**

```sql
-- ERP: 발주서
CREATE TABLE IF NOT EXISTS erp_purchase_orders (
    id          BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    workflow_id TEXT NOT NULL,
    quantity    INTEGER NOT NULL CHECK (quantity > 0),
    supplier    TEXT NOT NULL,
    status      TEXT NOT NULL DEFAULT 'pending',
    extra       TEXT NOT NULL DEFAULT 'null',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Blog: 글
CREATE TABLE IF NOT EXISTS blog_articles (
    id          BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    title       TEXT NOT NULL,
    body        TEXT NOT NULL DEFAULT '',
    category    TEXT NOT NULL,
    slug        TEXT NOT NULL DEFAULT '',
    extra       TEXT NOT NULL DEFAULT 'null',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Shop: 상품 주문
CREATE TABLE IF NOT EXISTS shop_orders (
    id          BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    product_id  TEXT NOT NULL,
    price       DOUBLE PRECISION NOT NULL CHECK (price >= 0),
    quantity    INTEGER NOT NULL CHECK (quantity > 0),
    status      TEXT NOT NULL DEFAULT 'pending',
    extra       TEXT NOT NULL DEFAULT 'null',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

### D4: DataStore 트레이트 확장

기존 `save_board_post`/`get_board_post`와 동일한 패턴으로:
- `save_erp_purchase_order` / `get_erp_purchase_order`
- `save_blog_article` / `get_blog_article`
- `save_shop_order` / `get_shop_order`

### D5: API 엔드포인트

기존 `/api/board`에 추가로:
- `POST /api/erp/purchase-orders`
- `POST /api/blog/articles`
- `POST /api/shop/orders`

---

## 2. 산출물

| 파일 | 내용 |
|------|------|
| `Dockerfile` | 멀티 스테이지 빌드 |
| `.dockerignore` | 빌드 제외 파일 |
| `src/domain.rs` | DomainPipeline 추상화 |
| `src/pipeline.rs` | BoardPipeline → DomainPipeline 위임 |
| `src/meta/store.rs` | ERP/Blog/Shop DataStore 메서드 추가 |
| `migrations/002_domain_tables.sql` | ERP/Blog/Shop 테이블 |
| `src/http.rs` | 신규 API 엔드포인트 |
| `tests/phase12_domain.rs` | DomainPipeline + 스키마 테스트 |

---

## 3. 주의사항

1. **BoardPipeline 호환성**: 기존 `/api/board` API는 그대로 유지 (DomainPipeline 위임)
2. **DataStore 트레이트**: 기존 구현체(MemoryStore, PostgresStore)에 메서드 추가 필요
3. **Dockerfile**: rquickjs-sys C 컴파일 필요 → `pkg-config` + `libssl-dev` 필수
4. **스키마**: `GENERATED ALWAYS AS IDENTITY`로 ID 자동 생성 (Phase 10과 동일 패턴)
