# HANDOVER: Phase 12 — 운영 배포 패키지 + DomainPipeline 추상화

> 이 문서는 Phase 12를 완료하고 단절했을 때, 다음 개발자(또는 다음 세션의 AI 에이전트)가
> 이어서 작업을 진행할 수 있도록 작성된 핸드오버 문서입니다.

---

## 1. Phase 12 완료 내용

### 1.1 Dockerfile

`Dockerfile` + `.dockerignore` 신규 생성:

```dockerfile
FROM rust:1.79-slim AS builder
# pkg-config + libssl-dev + build-essential (rquickjs-sys C 컴파일)
RUN cargo build --release

FROM debian:bookworm-slim
# ca-certificates + libssl3
COPY --from=builder target/release/meta_server /usr/local/bin/
COPY migrations/ scripts/
EXPOSE 3000
CMD ["meta_server"]
```

**빌드:**
```bash
docker build -t meta_server .
docker run -p 3000:3000 -e DATABASE_BACKEND=memory meta_server
```

### 1.2 DomainPipeline 추상화

`src/domain.rs` 신규 생성:

```rust
pub struct DomainPipeline {
    pub domain: String,           // "board", "erp", "blog", "shop"
    pub base_module_id: String,
    pub ext_module_id: String,
    pub loader: ModuleLoaderRef,
    pub cache: Arc<L1Cache>,
    pub store: Arc<dyn DataStore>,
    pub observability: Arc<Observability>,
}
```

- `DomainPipeline::build()`: Base + Extension 모듈 시드 + L1 캐시 + 옵저버
- `handle_request(payload)`: before_* 훅 → 저장 → after_* 훅
- 훅 이름 매핑: `before_create`/`after_save` (board), `before_process`/`after_process` (erp), `before_publish`/`after_publish` (blog), `before_order`/`after_order` (shop)

### 1.3 데이터 스키마

`migrations/002_domain_tables.sql` 신규 생성:

| 테이블 | 주요 컬럼 |
|--------|----------|
| `erp_purchase_orders` | workflow_id, quantity, supplier, status, extra |
| `blog_articles` | title, body, category, slug, extra |
| `shop_orders` | product_id, price, quantity, status, extra |

모두 `BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY` + `created_at TIMESTAMPTZ`.

### 1.4 DataStore 확장

`src/meta/store.rs`:
- `ErpPurchaseOrder`, `BlogArticle`, `ShopOrder` 엔티티 추가
- `DataStore` 트레이트에 6개 메서드 추가:
  - `save_erp_purchase_order` / `get_erp_purchase_order`
  - `save_blog_article` / `get_blog_article`
  - `save_shop_order` / `get_shop_order`
- `PostgresStore` + `MemoryStore` 구현

### 1.5 API 엔드포인트

`src/http.rs`:
- `POST /api/erp/purchase-orders` → `domain_create_erp`
- `POST /api/blog/articles` → `domain_create_blog`
- `POST /api/shop/orders` → `domain_create_shop`

`build_router` 시그니처에 `domains: Vec<Arc<DomainPipeline>>` 인자 추가.

### 1.6 전체 테스트: **77개 전부 통과**

| 테스트 파일 | 개수 |
|-------------|------|
| Phase 12 (domain) | 4 |
| 기존 (Phase 1~11) | 73 |
| **합계** | **77** |

---

## 2. 주의사항

### 2.1 build_router 시그니처 변경

```rust
// 변경 전
build_router(engine, board, socket_config, socket_script)

// 변경 후
build_router(engine, board, domains, socket_config, socket_script)
```

`domains: Vec<Arc<DomainPipeline>>` 인자 추가. 기존 테스트에서 `vec![]` 전달.

### 2.2 Docker 빌드

rquickjs-sys가 C 코드를 컴파일하므로:
- `pkg-config` + `libssl-dev` + `build-essential` 필수
- `debian:bookworm-slim` 런타임에는 `libssl3`만 필요

### 2.3 도메인별 훅 이름

| 도메인 | before_* | after_* |
|--------|----------|---------|
| board | `before_create` | `after_save` |
| erp | `before_process` | `after_process` |
| blog | `before_publish` | `after_publish` |
| shop | `before_order` | `after_order` |

Base 스크립트는 `api.<hook>`으로 정의, Extension은 `Base.<hook>`으로 체이닝.

### 2.4 BoardPipeline vs DomainPipeline

- `BoardPipeline`: 기존 구조, `/api/board` 엔드포인트에서 사용
- `DomainPipeline`: Phase 12 신규, `/api/erp/...`, `/api/blog/...`, `/api/shop/...`에서 사용
- Board는 호환성 유지, 추후 BoardPipeline → DomainPipeline으로 마이그레이션 가능

---

## 3. 다음 단계 (Phase 13)

### 3.1 운영 배포 검증
- WSL2 Docker로 `docker build` + `docker run` 실 테스트
- PostgreSQL + Redis 실제 연결
- `migrations/001_init.sql` + `002_domain_tables.sql` 자동 적용

### 3.2 모니터링
- `/api/health`에 도메인별 상태 추가
- Prometheus metrics (선택)

### 3.3 인증/인가
- JWT 토큰 인증
- 도메인별 권한 체크

---

## 4. 빌드 및 테스트

```bash
# 전체 테스트 (77개)
cargo test

# Phase 12 테스트만
cargo test --test phase12_domain

# Docker 빌드 (WSL2)
wsl -d Ubuntu -- docker build -t meta_server -f /mnt/c/ai/meta_server/Dockerfile /mnt/c/ai/meta_server
```
