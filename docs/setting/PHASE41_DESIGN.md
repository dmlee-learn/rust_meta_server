# Phase 41: 계층형 템플릿 시스템 (Design)

## 0. 목표
홈페이지/블로그/게시판/쇼핑몰에 콘텐츠를 입력할 때 **계층형 템플릿(페이지 → 메뉴 → 본문)** 을 활용해 빠르게 구성한다. 템플릿은 **파일 채널(`templates/*.json` 시드)** 과 **DB 채널(Admin API 런타임 등록)** 두 경로로 관리되며, 개선 시 **새 버전(고정)** 또는 **기존 인스턴스 동기화**를 사용자가 선택한다.

## 1. 확정 방향 (사용자 승인)
- 범위: **콘텐츠 템플릿 + 사이트 템플릿 + 계층(네이스티드 트리)**.
- 채널: **파일 + DB 양립** (JSON 파일 1개당 템플릿 1개).
- 인스턴스 저장: **기존 도메인 테이블 + `template_id` 컬럼**.
- 구현 분할: **단1(데이터 모델+채널+읽기 API)** → **단2(Q3 스태킹/적용+인스턴스 생성 통합)**.

## 2. 데이터 모델 — 계층형 템플릿 트리

### 2.1 `meta_templates` 테이블
```sql
CREATE TABLE meta_templates (
    template_id   TEXT NOT NULL,
    tenant_id     TEXT NOT NULL DEFAULT 'default',
    parent_id     TEXT,                         -- NULL=루트(page/site), 비NULL=하위(menu/body/field)
    name          TEXT NOT NULL,
    ttype         TEXT NOT NULL,                -- 'page'|'menu'|'body'|'field'|'site'
    domain        TEXT,                         -- 'board'|'blog'|'shop'|'homepage'|NULL
    payload       TEXT NOT NULL DEFAULT '{}',   -- JSON: 필드 스키마/레이아웃/기본값
    source        TEXT NOT NULL DEFAULT 'db',   -- 'db' | 'file'
    file_path     TEXT,                         -- source='file'일 때
    version       BIGINT NOT NULL DEFAULT 1,
    enabled       BOOLEAN NOT NULL DEFAULT TRUE,
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (template_id, tenant_id, version)
);
CREATE INDEX idx_meta_templates_active ON meta_templates(tenant_id, parent_id, enabled);
CREATE INDEX idx_meta_templates_domain ON meta_templates(tenant_id, domain);
```

**비고 (Phase 38 직렬화 특이점 회피):**
- 쓰기 쿼리는 `&String`(TEXT) 4개 + `f64`(updated_at) ≤ 5 파라미터.
- `parent_id`/`domain`/`file_path`는 NULL 가능 → **COALESCE로 비NULL 기본값**을 사용해 NULL 바인딩 피하기 (빈 문자열 또는 `'default'`).
- `payload`는 JSONB가 아닌 **TEXT**(Phase 38 패턴: JSONB 바인딩 회피).

### 2.2 템플릿 트리 예시
```
site.shop (ttype=site, parent=NULL, tenant=acme)
├── page.home (ttype=page, parent=site.shop, domain=homepage)
│   ├── menu.main (ttype=menu, parent=page.home)
│   └── body.hero (ttype=body, parent=page.home)
└── page.blog (ttype=page, parent=site.shop, domain=blog)
    └── body.post (ttype=body, parent=page.blog, domain=blog)
```

### 2.3 기존 도메인 테이블에 `template_id` 추가
```sql
ALTER TABLE board_posts ADD COLUMN IF NOT EXISTS template_id TEXT;
ALTER TABLE blog_articles ADD COLUMN IF NOT EXISTS template_id TEXT;
ALTER TABLE shop_products ADD COLUMN IF NOT EXISTS template_id TEXT;
-- 인스턴스가 어떤 템플릿(버전)에서 왔는지 기록. NULL = 템플릿 미사용.
```

## 3. 두 채널 (파일 + DB)

### 3.1 파일 채널 (`templates/*.json`)
- `templates/` 디렉토리에 JSON 파일 1개당 템플릿 1개.
- 파일 스키마:
```json
{
  "template_id": "page.home",
  "tenant_id": "default",
  "parent_id": "site.shop",
  "name": "홈 페이지",
  "ttype": "page",
  "domain": "homepage",
  "enabled": true,
  "payload": {
    "fields": [
      {"key": "hero_title", "type": "text", "label": "히어로 제목", "required": true},
      {"key": "hero_body", "type": "richtext", "label": "히어로 본문"}
    ],
    "layout": ["hero_title", "hero_body"]
  }
}
```
- **시드**: 시작 시(`migrate` 단계 또는 파이프라인 빌드) 파일 스캔 → `meta_templates`에 `UPSERT` (기존 `scripts/*.js` 시드와 동일한 패턴). `source='file'`, `file_path` 기록.

### 3.2 DB 채널 (Admin API)
- 런타임에 Admin API로 등록/수정. `source='db'`.
- **동일 `template_id` 시 DB가 파일보다 우선** (실행 시 상태가 진실). 파일은 시드/기본값 제공.

### 3.3 채널 우선순위 정리
- `GET` 시: 파일+DB 모두 있으면 **DB(실행 시) 우선**, 파일만 있으면 파일.
- 파일 시드는 **멱등** (같은 내용이면 재시드 안 됨, content hash 비교).

## 4. 읽기 API (단1)

| 메서드 | 경로 | 설명 |
|---|---|---|
| GET | `/api/templates/{id}/tree?tenant={t}` | 템플릿 노드 + 모든 자식(재귀) 결합 트리 |
| GET | `/api/templates?domain={d}&parent={p}&tenant={t}` | 조건별 목록 |
| GET | `/api/templates/{id}?tenant={t}` | 단일 노드 상세 (활성 버전) |

**트리 결합 응답:**
```json
{
  "code": "OK",
  "template": {
    "template_id": "site.shop", "name": "쇼핑몰", "ttype": "site",
    "children": [
      {"template_id": "page.home", "ttype": "page", "payload": {...},
       "children": [
         {"template_id": "menu.main", "ttype": "menu", "payload": {...}},
         {"template_id": "body.hero", "ttype": "body", "payload": {...}}
       ]},
      {"template_id": "page.blog", "ttype": "page", "payload": {...}, "children": [...]}
    ]
  }
}
```

## 5. 스태킹 / 인스턴스 생성 / Q3 (단2)

### 5.1 인스턴스 생성 시 템플릿 적용
- `POST /api/{domain}/items?template_id={id}` (단2):
  - 해당 템플릿 트리에서 `payload.fields`의 `default` 수집 → 입력폼 구조+기본값 반환.
  - 실제 저장 시 `template_id` 컬럼에 출처 기록 (Q3-1).
- **기본값 채움**: 필드의 `default`가 없으면 빈 값, 있으면 기본값.

### 5.2 Q3: 개선 시 사용자 선택
- `POST /api/admin/templates/{id}/bump` → **새 버전 등록** (기존 인스턴스 스냅샷 보존, `version+1`).
- `POST /api/admin/templates/{id}/propagate` → **기존 인스턴스 동기화**:
  - 대상: `template_id={id}`로 생성된 인스턴스 중 `sync_policy='live'`인 것만 payload 재적용.
  - `sync_policy='frozen'` 인스턴스는 보존 (고정).
- 인스턴스는 `template_id` + (선택) `template_version` + `sync_policy`로 관리.
  - `board_posts`/`blog_articles`/`shop_products`에 `template_version INT`, `sync_policy TEXT` 추가 (단2).

### 5.3 사이트 템플릿 프로비저닝 (단2, 선택 확장)
- `POST /api/admin/tenants/{id}/apply-site-template?template_id={id}`:
  - `site.*` 템플릿의 트리(모듈/카테고리/테마)를 해당 테넌트에 **스냅샷 복제**.

## 6. 에러 코드
| 코드 | 상황 | HTTP |
|---|---|---|
| `CJ1201` | 템플릿 입력 유효성 (필드 누락 등) | 422 |
| `CJ4001` | 템플릿 트리 순환 참조 감지 | 400 |
| `CR4006` | 템플릿 미존재 | 404 |
| `CR0005` | DB 쿼리 오류 | 500 |
| `CJ1104` | 템플릿 payload JSON 파싱 실패 | 400 |

## 7. 원칙 정합 (AGENTS.md)
- ✅ 재배포 불필요 (DB 채널 + 파일 시드).
- ✅ 불변 컨텍스트 (기본 스냅샷 고정, `propagate` 선택으로만 동기화).
- ✅ 메타데이터(JSON payload) vs 스크립트(검증 훅) 분리.
- ✅ Base 스코프 오염 없음 (템플릿은 읽기 전용 참조).
- ✅ Phase 38 직렬화 패턴 준수 (TEXT 바인딩, ≤5 파라미터).

## 8. 구현 분할
### 단1 (Step 1 완료)
- [x] `migrations/006_meta_templates.sql`: `meta_templates` 테이블 + `template_id` 컬럼 3개 테이블.
- [x] `src/meta/store.rs`: `MetaTemplate` 모델 + `DataStore` 템플릿 메서드 (list/get/children/tree).
- [x] `MemoryStore` + `PostgresStore` 구현.
- [x] 파일 시드: `templates/` 스캐너 (파일 → DB 시드, 멱등).
- [x] 읽기 API: `/api/templates/...` (단1의 3개 엔드포인트).
- [x] 테스트: `tests/phase41_template.rs` (6개).
- [ ] Docker 검증.

### 단2 (Step 2 완료)
- [x] 스태킹/인스턴스 생성 API (`POST /api/board/items?template_id=`).
- [x] Q3: `bump` + `propagate` + `sync_policy`.
- [x] 사이트 템플릿 프로비저닝.
- [x] 대시보드 UI (템플릿 트리 뷰).
- [x] OpenAPI 문서.
- [x] 테스트: 4개 신규 (총 133개).
