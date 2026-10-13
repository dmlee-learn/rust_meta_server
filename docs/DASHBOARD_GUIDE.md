# MetaServer 대시보드 사용 가이드

> `http://localhost:3000/admin` 접속 후 토큰(`admin-secret-123`)으로 로그인하면 아래 대시보드가 표시됩니다.

---

## 화면 구조

```
┌─────────────────────────────────────────────────────────────────┐
│  MetaServer Admin    [Status: UP]  Uptime: 00:05:23  [Logout]   │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─── KPI Cards ───────────────────────────────────────────┐   │
│  │ DB: memory/8m │ Cache: 12/100 │ Uptime: 00:05 │ Mem: 10MB │   │
│  └──────────────────────────────────────────────────────────┘   │
│                                                                 │
│  ┌─── Charts ──────────────────────────────────────────────┐   │
│  │ HTTP by Domain │ Cache Hit Rate │ Hook Exec vs Errors   │   │
│  └──────────────────────────────────────────────────────────┘   │
│                                                                 │
│  ┌─── System Status ───────────────────────────────────────┐   │
│  │ Database: postgres │ Migrations: 2 │ Engine: 8MB limit   │   │
│  └──────────────────────────────────────────────────────────┘   │
│                                                                 │
│  ┌─── Domain Heatmap ──────────────────────────────────────┐   │
│  │ board: 12 │ blog: 8 │ shop: 5 │ erp: 1                  │   │
│  └──────────────────────────────────────────────────────────┘   │
│                                                                 │
│  ┌─── Server Controls ─────────────────────────────────────┐   │
│  │ [Ping Trigger] [Export JSON]                             │   │
│  └──────────────────────────────────────────────────────────┘   │
│                                                                 │
│  ┌─── Templates (Hierarchical) ────────────────────────────┐   │
│  │  [Load Templates]                                       │   │
│  │  Tenant: [default]  ID: [body.blog_post]  Domain: [all] │   │
│  │  [Tree] [Bump] [Propagate]                               │   │
│  │  ──────────────────────────────────────────────────────  │   │
│  │  Template List (8 items)                                 │   │
│  │  ──────────────────────────────────────────────────────  │   │
│  │  Tree View                                               │   │
│  │  ──────────────────────────────────────────────────────  │   │
│  │  Instance Creation                                       │   │
│  │  Template: [body.blog_post] Domain: [blog] Policy: [live]│   │
│  │  Title: [optional]                                       │   │
│  │  [Create Instance]                                       │   │
│  │  ──────────────────────────────────────────────────────  │   │
│  │  Site Provisioning                                       │   │
│  │  Tenant: [acme]  Site: [site.shop]                       │   │
│  │  [Apply]                                                 │   │
│  └──────────────────────────────────────────────────────────┘   │
│                                                                 │
│  ┌─── Recent Errors ───────────────────────────────────────┐   │
│  │ [Flush Cache]                                           │   │
│  └──────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

---

## 1. KPI Cards (상단)

자동으로 5초마다 갱신됩니다.

| 카드 | 설명 |
|---|---|
| **DB** | 백엔드 유형 + 적용된 마이그레이션 수 |
| **Cache** | L1 캐시 hit/total |
| **Uptime** | 서버 실행 시간 |
| **Memory** | rquickjs 엔진 사용 메모리 / 한도 |

---

## 2. Charts (중단)

Chart.js 기반 3개 차트.

| 차트 | 설명 |
|---|---|
| **HTTP Requests by Domain** | board/blog/shop/erp/erp별 요청 수 막대 |
| **Cache Hit Rate** | 전체 캐시 hit/miss 비율 |
| **Hook Executions vs Errors** | JS 훅 실행 수와 에러 수 비교 |

---

## 3. Templates (Hierarchical)

### 3.1 템플릿 목록

1. `Tenant` 입력란에 `default` 입력 (비워두면 default)
2. **Load Templates** 버튼 클릭
3. 8개 템플릿 목록 표시

```
site.shop (site) v1
page.home (page) v1
menu.main (menu) v1
menu.sub_categories (menu) v1
menu.sub_products (menu) v1
body.blog_post (body) v1
body.hero (body) v1
body.footer (body) v1
```

### 3.2 템플릿 트리

1. `ID` 입력란에 `site.shop` 입력
2. `Tenant` 입력란에 `default` 입력
3. **Tree** 버튼 클릭
4. 나무 구조 표시:

```
site.shop (site)
├── page.home (page)
│   └── menu.main (menu)
│       └── menu.sub_categories (menu)
├── menu.main (menu)
│   └── menu.sub_products (menu)
├── body.blog_post (body)
├── body.hero (body)
└── body.footer (body)
```

### 3.3 Bump (버전 증분)

템플릿을 수정한 후 새 버전을 만들 때 사용합니다.

1. `ID` 입력란에 `body.blog_post` 입력
2. **Bump** 버튼 클릭
3. 결과: `new_version: 2`

> 기존 인스턴스는 영향을 받지 않습니다. `sync_policy=live`인 인스턴스만 전파 대상입니다.

### 3.4 Propagate (전파)

새 버전을 `sync_policy=live`인 인스턴스에 적용합니다.

1. `ID` 입력란에 `body.blog_post` 입력
2. **Propagate** 버튼 클릭
3. 결과: `propagated: 2` (2개 live 인스턴스 갱신)

> `frozen` 인스턴스는 전파 대상에서 제외됩니다.

---

## 4. Instance Creation (인스턴스 생성)

### 4.1 생성 절차

1. **Load Templates** 버튼으로 템플릿 목록 로드
2. **Template** dropdown에서 body 템플릿 선택 (예: `body.blog_post`)
3. **Domain** 선택: `blog`
4. **Policy** 선택: `live` (템플릿 변경 시 자동 전파) 또는 `frozen` (버전 고정)
5. **Title** 입력 (선택, 비워두면 템플릿 default 사용)
6. **Create Instance** 버튼 클릭

### 4.2 결과 예시

```
✓ Created blog instance: id=5, template=body.blog_post v1, policy=live
```

### 4.3 도메인별 템플릿

| 도메인 | 사용할 수 있는 템플릿 |
|---|---|
| **blog** | `body.blog_post` |
| **board** | `body.hero`, `body.footer` |
| **shop** | `body.hero` |

> body 템플릿만 인스턴스 생성에 사용할 수 있습니다. site/page/menu는 컨테이너 노드입니다.

---

## 5. Site Provisioning (사이트 프로비저닝)

새 테넌트에 사이트 템플릿의 body 노드를 복제합니다.

1. **Target Tenant** 입력란에 `acme` 입력
2. **Site Template** 입력란에 `site.shop` 입력
3. **Apply** 버튼 클릭
4. 결과: `copied: 3` (3개 body 노드 복제: blog_post, hero, footer)

> acme 테넌트에서 이제 `body.blog_post`, `body.hero`, `body.footer` 템플릿을 사용할 수 있습니다.

---

## 6. Server Controls

| 버튼 | 설명 |
|---|---|
| **Ping Trigger** | DB/Redis latency 측정 |
| **Export JSON** | 서버 상태 전체 JSON 다운로드 |

---

## 7. Recent Errors

최근 50개 에러 로그 표시.

| 열 | 설명 |
|---|---|
| **Time** | 에러 발생 시각 |
| **Code** | 에러 코드 (예: CR4006) |
| **Message** | 에러 내용 |
| **Domain** | 발생 도메인 (board/blog/shop) |

**Flush Cache** 버튼: L1 인메모리 캐시 전체 삭제.

---

## 8. Extension (모듈) Management

Phase 40부터 추가된 탭.

| 기능 | 설명 |
|---|---|
| **Module List** | `base`/`extension` 모듈 목록 |
| **Toggle** | 모듈 enable/disable |
| **Reload** | 모듈 스크립트 재로드 |
| **Deploy** | 새 모듈 스크립트 배포 (compile validation 포함) |

토큰 입력란에 `admin-secret-123`을 입력해야 API 인증이 가능합니다.

---

## 9. Logout

크롬에서 `localStorage`의 `admin_token`을 삭제하면 로그아웃 됩니다.
아니면 브라우저 창을 닫으면 됩니다 (sessionStorage 방식).

---

## 빠른 테스트 시나리오

```
1. Load Templates → 8개 확인
2. Tree (site.shop) → 8개 노드 확인
3. Instance Creation:
   - body.blog_post + blog + live + "My Blog Post" → Create
   - body.hero + board + frozen + "Hero Banner" → Create
   - body.hero + shop + frozen + "New Product" → Create
4. Bump (body.blog_post) → new_version: 2
5. Propagate → propagated: 1 (live 인스턴스만)
6. Site Provisioning: acme + site.shop → copied: 3
7. API 확인: GET /api/blog/articles → 5개 글 (기존 4개 + 새 1개)
```
