# MetaServer 접속 가이드

> 이 문서는 MetaServer에 브라우저와 API로 접속하는 방법을 안내합니다.
> `{컴퓨터 IP}`는 WSL 또는 Docker가 실행 중인 머신의 IP로 치환하세요.

---

## 1. WSL IP 확인

WSL 내부에서 아래 명령으로 IP를 확인합니다.

```bash
wsl -d Ubuntu -- hostname -I
```

첫 번째 IP가 WSL 내부 IP입니다. 예: `172.31.159.72`.

> **참고:** WSL을 재시작하면 IP가 변경될 수 있습니다.

---

## 2. 접속 URL

| 항목 | URL |
|---|---|
| **관리자 대시보드** | `http://{컴퓨터 IP}:3000/admin` |
| **헬스체크** | `http://{컴퓨터 IP}:3000/health` |
| **Grafana 대시보드** | `http://{컴퓨터 IP}:3001` |
| **Prometheus** | `http://{컴퓨터 IP}:9090` |

---

## 3. 계정 정보

| 항목 | 값 |
|---|---|
| **관리자 토큰** | `admin-secret-123` |
| **JWT** | 비활성화 (`JWT_DISABLED=true`) |
| **Grafana 계정** | `admin` / `admin` |
| **PostgreSQL** | `meta` / `metapass` |
| **Redis** | 비밀번호 `metapass` |

### 3.1 로그인

관리자 대시보드(`/admin`)에 접속하면 로그인 페이지가 표시됩니다.

1. `http://localhost:3000/admin` 접속
2. **Admin Token** 입력: `admin-secret-123`
3. **로그인** 클릭 → 대시보드 표시

토큰은 `sessionStorage`에 저장됩니다. 브라우저 창을 닫으면 만료됩니다.

> **참고:** `ADMIN_TOKEN` 환경변수가 미설정된 경우(개발 모드), 로그인 없이 대시보드에 바로 접근 가능합니다.

### 관리자 API 인증

모든 `/api/admin/*` 요청에 헤더를 추가해야 합니다.

```
X-Admin-Token: admin-secret-123
```

공개 API(`GET /api/templates`, `GET /api/blog/articles` 등)는 인증이 필요하지 않습니다.

---

## 4. API 엔드포인트

### 4.1 헬스체크

```bash
curl http://{컴퓨터 IP}:3000/health
```

응답 예시:

```json
{
  "status": "ok",
  "database": { "backend": "postgres", "migrations_applied": ["001_init.sql", "002_domain_tables.sql"] },
  "redis": { "enabled": true },
  "pipelines": { "blog": "initialized", "board": "initialized", "erp": "initialized", "shop": "initialized" }
}
```

### 4.2 템플릿

| 메서드 | 경로 | 설명 |
|---|---|---|
| GET | `/api/templates` | 템플릿 목록 (쿼리: `?tenant=default&domain=&parent=`) |
| GET | `/api/templates/:id` | 템플릿 상세 |
| GET | `/api/templates/:id/tree` | 템플릿 트리 (자식 노드 포함) |

```bash
# 템플릿 목록
curl "http://{컴퓨터 IP}:3000/api/templates?tenant=default"

# 트리 (site.shop와 모든 하위 노드)
curl "http://{컴퓨터 IP}:3000/api/templates/site.shop/tree?tenant=default"
```

### 4.3 인스턴스 생성 (템플릿 기반)

| 메서드 | 경로 | 도메인 |
|---|---|---|
| POST | `/api/board/items` | 게시판 |
| POST | `/api/blog/items` | 블로그 |
| POST | `/api/shop/items` | 쇼핑몰 |

쿼리 파라미터: `template_id` (필수), `sync_policy` (선택: `frozen`/`live`, 기본 `frozen`)

```bash
# 게시판 인스턴스
curl -X POST "http://{컴퓨터 IP}:3000/api/board/items?template_id=body.footer&sync_policy=live" \
  -H "Content-Type: application/json" \
  -d '{"title":"새 공지사항"}'

# 블로그 인스턴스
curl -X POST "http://{컴퓨터 IP}:3000/api/blog/items?template_id=body.blog_post&sync_policy=live" \
  -H "Content-Type: application/json" \
  -d '{"title":"새 블로그 글"}'

# 쇼핑몰 인스턴스
curl -X POST "http://{컴퓨터 IP}:3000/api/shop/items?template_id=body.hero&sync_policy=frozen" \
  -H "Content-Type: application/json" \
  -d '{"name":"새 상품"}'
```

### 4.4 블로그 글 조회

```bash
# 전체 목록
curl "http://{컴퓨터 IP}:3000/api/blog/articles"
```

### 4.5 템플릿 버전 관리 (관리자)

| 메서드 | 경로 | 설명 |
|---|---|---|
| POST | `/api/admin/templates/:id/bump` | 템플릿 버전 증분 |
| POST | `/api/admin/templates/:id/propagate` | 템플릿 전파 (쿼리: `?domain=all\|board\|blog\|shop`) |
| POST | `/api/admin/tenants/:id/apply-site-template` | 사이트 프로비저닝 (쿼리: `?site_template_id=`) |

```bash
# 버전 증분
curl -X POST "http://{컴퓨터 IP}:3000/api/admin/templates/body.footer/bump?tenant=default" \
  -H "X-Admin-Token: admin-secret-123"

# 전파 (게시판 live 인스턴스)
curl -X POST "http://{컴퓨터 IP}:3000/api/admin/templates/body.footer/propagate?tenant=default&domain=board" \
  -H "X-Admin-Token: admin-secret-123"

# 사이트 프로비저닝 (acme 테넌트에 body 노드 복제)
curl -X POST "http://{컴퓨터 IP}:3000/api/admin/tenants/acme/apply-site-template?site_template_id=site.shop&tenant=default" \
  -H "X-Admin-Token: admin-secret-123"
```

### 4.6 관리자 상태

```bash
# 서버 상태
curl "http://{컴퓨터 IP}:3000/api/admin/status" -H "X-Admin-Token: admin-secret-123"

# 메트릭
curl "http://{컴퓨터 IP}:3000/api/admin/metrics" -H "X-Admin-Token: admin-secret-123"

# 에러 로그
curl "http://{컴퓨터 IP}:3000/api/admin/logs" -H "X-Admin-Token: admin-secret-123"
```

---

## 5. 관리자 대시보드 사용 가이드

브라우저에서 `http://{컴퓨터 IP}:3000/admin`으로 접속합니다.

### 5.1 Dashboard 탭

- 서버 상태 (DB/Redis/Engine)
- 실시간 메트릭 (CPU, 메모리, 요청 수)
- 에러 로그

### 5.2 Templates (Hierarchical) 탭

- **템플릿 목록:** 8개 템플릿 (site/page/menu/body)
- **트리 뷰:** `site.shop` → `page.home` → `menu.main` → `body.*`
- **Bump:** 템플릿 버전 증분
- **Propagate:** 버전 전파 (도메인 선택: all/board/blog/shop)

### 5.3 Instance Creation 탭

- **템플릿 선택:** body 템플릿만 dropdown에 표시
- **도메인 선택:** board / blog / shop
- **Sync Policy:** frozen (버전 고정) / live (자동 전파)
- **Title/Name:** 인스턴스 제목 (템플릿 default가 기본값)
- **생성:** 클릭 후 결과 표시

### 5.4 Site Provisioning

- **Target Tenant:** 대상 테넌트 ID
- **Site Template:** `site.shop` 선택
- **Apply:** 대상 테넌트에 body 노드 복제

---

## 6. 샘플 템플릿 구조

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

---

## 7. troubleshooting

### 7.1 `localhost`로 접속이 안 될 때

WSL2의 Docker 네트워크는 Windows의 `localhost`와 연결되지 않습니다.
반드시 `{컴퓨터 IP}`(WSL IP)를 사용하세요.

```bash
# WSL IP 확인
wsl -d Ubuntu -- hostname -I
# 첫 번째 IP 사용: http://172.31.159.72:3000/admin
```

### 7.2 컨테이너가 안 올라올 때

```bash
# 컨테이너 상태 확인
wsl -d Ubuntu -- docker ps

# 로그 확인
wsl -d Ubuntu -- docker logs meta_server-meta_server-1 --tail 50

# 재시작
wsl -d Ubuntu -- docker compose up -d meta_server
```

### 7.3 템플릿이 없을 때

컨테이너에 `templates/` 디렉토리가 없는 경우:

```bash
# Dockerfile에 templates/ COPY가 있는지 확인
grep "templates" Dockerfile

# 재빌드
wsl -d Ubuntu -- docker build -t meta_server:latest .
wsl -d Ubuntu -- docker compose up -d meta_server
```

### 7.4 DB 마이그레이션이 안 될 때

```bash
# 테이블 목록 확인
wsl -d Ubuntu -- docker exec meta_server-postgres-1 \
  psql -U meta -d meta \
  -c "SELECT table_name FROM information_schema.tables WHERE table_schema='public' ORDER BY table_name;"

# meta_templates 테이블이 없으면 수동 적용
wsl -d Ubuntu -- docker exec -i meta_server-postgres-1 \
  psql -U meta -d meta < migrations/006_meta_templates.sql
```

---

## 8. 기능별 테이블 스키마

| 도메인 | 스키마 파일 | 테이블 |
|---|---|---|
| 게시판 | `scripts/schemas/board.sql` | `board_posts`, `board_tags` |
| 블로그 | `scripts/schemas/blog.sql` | `blog_articles`, `blog_comments` |
| 쇼핑몰 | `scripts/schemas/shop.sql` | `shop_products`, `shop_carts`, `shop_orders` |
| 메타 | `scripts/schemas/meta.sql` | `meta_modules`, `meta_templates` |
| ERP | `scripts/schemas/erp.sql` | `erp_purchase_orders` |
