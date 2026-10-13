# MetaServer 사용 가이드

> 이 문서는 MetaServer를 개발·운영할 때 필요한 모든 정보를 담습니다.
> `{컴퓨터 IP}`는 WSL 또는 Docker가 실행 중인 머신의 IP로 치환하세요.

---

## 목차

1. [서버 실행](#1-서버-실행)
2. [접속 정보](#2-접속-정보)
3. [관리자 로그인](#3-관리자-로그인)
4. [API 사용](#4-api-사용)
5. [템플릿 시스템](#5-템플릿-시스템)
6. [인스턴스 생성](#6-인스턴스-생성)
7. [블로그/게시판/쇼핑몰](#7-블로그게시판쇼핑몰)
8. [관리자 대시보드](#8-관리자-대시보드)
9. [기능별 테이블 스키마](#9-기능별-테이블-스키마)
10. [Troubleshooting](#10-troubleshooting)

---

## 1. 서버 실행

### 1.1 Windows에서 직접 실행 (권장)

```batch
C:\Users\USER\run_meta_server.bat
```

MemoryStore + Windows 직접 실행으로 `http://localhost:3000`에 즉시 접속 가능합니다.
데이터는 서버 재시작 시 초기화됩니다.

### 1.2 Docker (WSL) 실행

```bash
# WSL에서 Docker 컨테이너 시작
wsl -d Ubuntu -- docker compose up -d meta_server
```

PostgreSQL 영구 저장 + Redis 캐시 활성화.
접속: `http://{컴퓨터 IP}:3000`

### 1.3 서버 구성

| 구성 요소 | Windows 모드로 | Docker 모드로 |
|---|---|---|
| DB | MemoryStore | PostgreSQL |
| Redis | 비활성 | 활성화 |
| 포트 | 3000 | 3000 |
| JWT | 비활성 | 비활성 |
| 템플릿 | 파일 시드 | 파일 시드 + DB |

---

## 2. 접속 정보

| 항목 | Windows 모드 | Docker 모드 |
|---|---|---|
| **관리자 페이지** | `http://localhost:3000/admin` | `http://{컴퓨터 IP}:3000/admin` |
| **헬스체크** | `http://localhost:3000/health` | `http://{컴퓨터 IP}:3000/health` |
| **블로그 API** | `http://localhost:3000/api/blog/articles` | `http://{컴퓨터 IP}:3000/api/blog/articles` |
| **템플릿 API** | `http://localhost:3000/api/templates` | `http://{컴퓨터 IP}:3000/api/templates` |
| **Grafana** | - | `http://{컴퓨터 IP}:3001` |
| **Prometheus** | - | `http://{컴퓨터 IP}:9090` |

### WSL IP 확인

```bash
wsl -d Ubuntu -- hostname -I
# 첫 번째 IP 사용 (예: 172.31.159.72)
```

> WSL을 재시작하면 IP가 변경될 수 있습니다.

---

## 3. 관리자 로그인

1. `http://localhost:3000/admin` 접속
2. **Admin Token** 입력: `admin-secret-123`
3. **로그인** 클릭

토큰은 `sessionStorage`에 저장됩니다. 브라우저 창을 닫으면 만료됩니다.

> `ADMIN_TOKEN` 환경변수 미설정 시(개발 모드) 로그인 없이 대시보드에 바로 접근 가능합니다.

### 계정 정보

| 항목 | 값 |
|---|---|
| **관리자 토큰** | `admin-secret-123` |
| **PostgreSQL** | `meta` / `metapass` |
| **Redis** | 비밀번호 `metapass` |
| **Grafana** | `admin` / `admin` |

---

## 4. API 사용

### 4.1 인증

관리자 API(`/api/admin/*`)에는 헤더를 추가해야 합니다.

```
X-Admin-Token: admin-secret-123
```

공개 API(`GET /api/templates`, `GET /api/blog/articles` 등)는 인증이 필요하지 않습니다.

### 4.2 엔드포인트 요약

| 메서드 | 경로 | 인증 | 설명 |
|---|---|---|---|
| GET | `/health` | 불필요 | 헬스체크 |
| GET | `/admin` | 로그인 | 대시보드 |
| GET | `/api/templates` | 불필요 | 템플릿 목록 |
| GET | `/api/templates/:id` | 불필요 | 템플릿 상세 |
| GET | `/api/templates/:id/tree` | 불필요 | 템플릿 트리 |
| POST | `/api/board/items` | 불필요 | 게시판 인스턴스 생성 |
| POST | `/api/blog/items` | 불필요 | 블로그 인스턴스 생성 |
| POST | `/api/shop/items` | 불필요 | 쇼핑몰 인스턴스 생성 |
| GET | `/api/blog/articles` | 불필요 | 블로그 글 목록 |
| POST | `/api/admin/templates/:id/bump` | 필요 | 템플릿 버전 증분 |
| POST | `/api/admin/templates/:id/propagate` | 필요 | 템플릿 전파 |
| POST | `/api/admin/tenants/:id/apply-site-template` | 필요 | 사이트 프로비저닝 |
| GET | `/api/admin/status` | 필요 | 서버 상태 |
| GET | `/api/admin/metrics` | 필요 | 메트릭 |
| GET | `/api/admin/logs` | 필요 | 에러 로그 |

### 4.3 사용 예시

```bash
# 헬스체크
curl http://localhost:3000/health

# 템플릿 목록
curl "http://localhost:3000/api/templates?tenant=default"

# 템플릿 트리
curl "http://localhost:3000/api/templates/site.shop/tree?tenant=default"

# 블로그 글 목록
curl "http://localhost:3000/api/blog/articles"

# 게시판 인스턴스 생성
curl -X POST "http://localhost:3000/api/board/items?template_id=body.footer&sync_policy=live" \
  -H "Content-Type: application/json" \
  -d '{"title":"새 공지사항"}'

# 블로그 인스턴스 생성
curl -X POST "http://localhost:3000/api/blog/items?template_id=body.blog_post&sync_policy=live" \
  -H "Content-Type: application/json" \
  -d '{"title":"새 블로그 글"}'

# 쇼핑몰 인스턴스 생성
curl -X POST "http://localhost:3000/api/shop/items?template_id=body.hero&sync_policy=frozen" \
  -H "Content-Type: application/json" \
  -d '{"name":"새 상품"}'

# 템플릿 버전 증분
curl -X POST "http://localhost:3000/api/admin/templates/body.footer/bump?tenant=default" \
  -H "X-Admin-Token: admin-secret-123"

# 템플릿 전파 (게시판)
curl -X POST "http://localhost:3000/api/admin/templates/body.footer/propagate?tenant=default&domain=board" \
  -H "X-Admin-Token: admin-secret-123"

# 사이트 프로비저닝
curl -X POST "http://localhost:3000/api/admin/tenants/acme/apply-site-template?site_template_id=site.shop&tenant=default" \
  -H "X-Admin-Token: admin-secret-123"
```

---

## 5. 템플릿 시스템

### 5.1 샘플 템플릿 구조

```
site.shop (site, v1)
├── page.home (page, v1)
│   └── menu.main (menu, v1)
│       └── menu.sub_categories (menu, v1)
├── menu.main (menu, v1)
│   └── menu.sub_products (menu, v1)
├── body.blog_post (body, v1, domain: blog)
├── body.hero (body, v1, domain: board)
└── body.footer (body, v1, domain: board)
```

| 템플릿 ID | 유형 | 도메인 | 설명 |
|---|---|---|---|
| `site.shop` | site | - | 쇼핑몰 사이트 루트 |
| `page.home` | page | - | 홈 페이지 |
| `menu.main` | menu | - | 메인 메뉴 |
| `menu.sub_categories` | menu | board | 카테고리 서브 메뉴 |
| `menu.sub_products` | menu | board | 상품 서브 메뉴 |
| `body.blog_post` | body | blog | 블로그 글 본문 |
| `body.hero` | body | board | 히어로 배너 |
| `body.footer` | body | board | 푸터 |

### 5.2 템플릿 파일

`templates/` 디렉토리에 JSON 파일로 저장됩니다. 서버 시작 시 자동으로 DB에 시드됩니다.

```
templates/
├── site.shop.json
├── page.home.json
├── menu.main.json
├── menu.sub_categories.json
├── menu.sub_products.json
├── body.blog_post.json
├── body.hero.json
└── body.footer.json
```

### 5.3 버전 관리

- **Bump**: 템플릿 버전 +1 (새 버전 생성, 기존 인스턴스 영향 없음)
- **Propagate**: `sync_policy=live`인 인스턴스에 새 버전 전파
- **sync_policy**:
  - `frozen` (기본): 인스턴스 생성 시점의 템플릿 버전 고정
  - `live`: 템플릿 변경 시 자동 전파

---

## 6. 인스턴스 생성

### 6.1 API로 생성

```bash
# 게시판
curl -X POST "http://localhost:3000/api/board/items?template_id=body.footer&sync_policy=live" \
  -H "Content-Type: application/json" \
  -d '{"title":"공지사항"}'

# 블로그
curl -X POST "http://localhost:3000/api/blog/items?template_id=body.blog_post&sync_policy=live" \
  -H "Content-Type: application/json" \
  -d '{"title":"블로그 글"}'

# 쇼핑몰
curl -X POST "http://localhost:3000/api/shop/items?template_id=body.hero&sync_policy=frozen" \
  -H "Content-Type: application/json" \
  -d '{"name":"상품"}'
```

### 6.2 대시보드로 생성

1. `http://localhost:3000/admin` → 로그인
2. **Templates (Hierarchical)** 섹션에서 **Load Templates** 클릭
3. **Instance Creation** 섹션에서:
   - 템플릿 선택 (body 템플릿만)
   - 도메인 선택 (board/blog/shop)
   - Policy 선택 (frozen/live)
   - Title 입력 (선택)
   - **Create Instance** 클릭

상세 가이드: `docs/DASHBOARD_GUIDE.md`

---

## 7. 블로그/게시판/쇼핑몰

### 7.1 블로그

| API | 설명 |
|---|---|
| `GET /api/blog/articles` | 블로그 글 전체 목록 |
| `POST /api/blog/articles` | 일반 블로그 글 생성 (파이프라인) |
| `POST /api/blog/items?template_id=...` | 템플릿 기반 블로그 글 생성 |
| `POST /api/blog/comments` | 블로그 댓글 생성 |
| `GET /api/blog/comments?article_id=...` | 블로그 글별 댓글 조회 |

### 7.2 게시판

| API | 설명 |
|---|---|
| `POST /api/board` | 일반 게시글 생성 (파이프라인) |
| `POST /api/board/items?template_id=...` | 템플릿 기반 게시글 생성 |

### 7.3 쇼핑몰

| API | 설명 |
|---|---|
| `POST /api/shop/orders` | 주문 생성 |
| `POST /api/shop/items?template_id=...` | 템플릿 기반 상품 생성 |

---

## 8. 관리자 대시보드

상세 가이드: `docs/DASHBOARD_GUIDE.md`

### 탭별 요약

| 섹션 | 기능 |
|---|---|
| **KPI Cards** | DB/Cache/Uptime/Memory 실시간 표시 |
| **Charts** | HTTP 도메인별 요청, 캐시 hit rate, 훅 에러 |
| **System Status** | DB 백엔드, 마이그레이션, 엔진 설정 |
| **Domain Heatmap** | 도메인별 요청 수 |
| **Server Controls** | Ping Trigger, Export JSON |
| **Templates** | 템플릿 목록/트리/Bump/Propagate |
| **Instance Creation** | 템플릿 기반 인스턴스 생성 |
| **Site Provisioning** | 테넌트별 사이트 프로비저닝 |
| **Recent Errors** | 최근 50개 에러 로그 + 캐시 flush |
| **Extension** | 모듈 목록/토글/리로드/배포 |

---

## 9. 기능별 테이블 스키마

| 도메인 | 스키마 파일 | 테이블 |
|---|---|---|
| 게시판 | `scripts/schemas/board.sql` | `board_posts`, `board_tags` |
| 블로그 | `scripts/schemas/blog.sql` | `blog_articles`, `blog_comments` |
| 쇼핑몰 | `scripts/schemas/shop.sql` | `shop_products`, `shop_carts`, `shop_orders` |
| 메타 | `scripts/schemas/meta.sql` | `meta_modules`, `meta_templates` |
| ERP | `scripts/schemas/erp.sql` | `erp_purchase_orders` |

### 마이그레이션 파일

```
migrations/
├── 001_init.sql          # meta_modules, board_posts
├── 002_domain_tables.sql # erp_purchase_orders, blog_articles, shop_orders
├── 003_tags_comments.sql # board_tags, blog_comments
├── 004_tenant.sql        # tenant_id 컬럼 추가
├── 005_shop_products.sql # shop_products, shop_carts
├── 006_meta_templates.sql # meta_templates, template_id 컬럼
└── 007_template_instance.sql # template_version, sync_policy 컬럼
```

---

## 10. Troubleshooting

### 10.1 `localhost`로 접속이 안 될 때 (Docker 모드)

WSL2의 Docker 네트워크는 Windows의 `localhost`와 연결되지 않습니다.
`{컴퓨터 IP}`(WSL IP)를 사용하세요.

```bash
# WSL IP 확인
wsl -d Ubuntu -- hostname -I
# http://172.31.159.72:3000/admin
```

또는 Windows에서 직접 실행하면 `localhost`로 즉시 접근 가능합니다.

### 10.2 컨테이너가 안 올라올 때

```bash
wsl -d Ubuntu -- docker ps
wsl -d Ubuntu -- docker logs meta_server-meta_server-1 --tail 50
wsl -d Ubuntu -- docker compose up -d meta_server
```

### 10.3 템플릿이 없을 때

```bash
grep "templates" Dockerfile
wsl -d Ubuntu -- docker build -t meta_server:latest .
wsl -d Ubuntu -- docker compose up -d meta_server
```

### 10.4 DB 마이그레이션이 안 될 때

```bash
wsl -d Ubuntu -- docker exec meta_server-postgres-1 \
  psql -U meta -d meta \
  -c "SELECT table_name FROM information_schema.tables WHERE table_schema='public' ORDER BY table_name;"

# meta_templates 테이블이 없으면
wsl -d Ubuntu -- docker exec -i meta_server-postgres-1 \
  psql -U meta -d meta < migrations/006_meta_templates.sql
```

### 10.5 로그인 시 토큰 오류

- `ADMIN_TOKEN` 환경변수가 `admin-secret-123`으로 설정되어 있는지 확인
- 로그인 페이지에서 토큰을 정확히 입력
- 브라우저 cache/CSS를 비우기 (Ctrl+Shift+R)

### 10.6 인스턴스 생성 시 category 오류

블로그 인스턴스 생성 시 `category` 필드가 필요합니다.
템플릿 `body.blog_post`의 default category는 `general`입니다.

```bash
# category 명시 지정
curl -X POST "http://localhost:3000/api/blog/items?template_id=body.blog_post&sync_policy=live" \
  -H "Content-Type: application/json" \
  -d '{"title":"Test","category":"general"}'
```

---

## 문서 indeks

| 문서 | 내용 |
|---|---|
| `docs/ACCESS_GUIDE.md` | 접속 정보, 계정, API 엔드포인트 |
| `docs/DASHBOARD_GUIDE.md` | 대시보드 화면별 사용 가이드 |
| `docs/STATUS.md` | Phase별 진행 상태 |
| `docs/plan/PHASE42_PLAN.md` | Phase 42 (P1-P5) 계획 |
| `docs/openapi.yaml` | OpenAPI 스펙 (Swagger) |
| `docs/setting/HANDOVER_phase_41_template.md` | Phase 41 핸드오버 |
