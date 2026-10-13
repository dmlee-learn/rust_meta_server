# MetaServer

**Axum + Tokio + rquickjs** 기반의 고성능 하이브리드 계층형 모듈러 서버.

인프라의 제어권은 Rust가 쥐고, 비즈니스 정책의 제어권은 스크립트에 위임한다.

---

## 아키텍처

```
┌─────────────────────────────────────────────────────────────┐
│                      MetaServer                              │
│                                                             │
│  ┌─────────────┐  ┌──────────────┐  ┌───────────────────┐  │
│  │  Axum (HTTP) │  │  Tokio (async) │  │  rquickjs (JS)    │  │
│  │  HTTP/WS API  │  │  Runtime/Mulit│  │  Sandboxed Context │  │
│  └──────┬───────┘  │  iplexing     │  └────────┬──────────┘  │
│         │           └──────┬───────┘           │             │
│         │                  │                   │             │
│  ┌──────┴──────────────────┴───────────────────┴──────────┐  │
│  │                     Pipeline Chain                       │  │
│  │   BoardPipeline → BlogPipeline → ShopPipeline → ErpPipe │  │
│  └──────┬──────────────────────────────────────────────────┘  │
│         │                                                      │
│  ┌──────┴──────────────────────────────────────────────────┐  │
│  │                    DataStore (Trait)                     │  │
│  │   MemoryStore (개발)  │  PostgresStore (운영)            │  │
│  └──────┬──────────────────────────────────────────────────┘  │
│         │                                                      │
│  ┌──────┴──────────────────────────────────────────────────┐  │
│  │              3-Level Cache (L1→L2→L3)                   │  │
│  │   L1: In-memory  ← Redis Pub/Sub →  L2: Redis           │  │
│  │                              →  L3: PostgreSQL          │  │
│  └─────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

---

## 프로젝트 구조

```
meta_server/
├── src/
│   ├── core/        # Axum 웹 인프라, Tokio 비동기 런타임
│   ├── engine/      # rquickjs 샌드박스, 메모리/CPU 제한
│   ├── meta/        # PostgreSQL 스키마, Redis 캐시, 템플릿
│   ├── sdk/         # JS에서 호출하는 Rust SDK (rust_ 접두사)
│   ├── http.rs      # API 핸들러, 라우팅
│   ├── admin.rs     # 관리자 대시보드 HTML (Vanilla JS + Chart.js)
│   ├── auth.rs      # JWT, admin token, CORS 미들웨어
│   ├── cache.rs     # L1/L2 캐시, Pub/Sub
│   ├── main.rs      # 서버 시작
│   └── ...
├── scripts/
│   ├── base/        # 기본 뼈대 파이프라인 (기판 CRUD)
│   ├── extensions/  # 도메인 확장 스크립트 (공지사항, QnA, etc.)
│   └── schemas/     # 기능별 테이블 스키마 (board/blog/shop/meta/erp)
├── templates/       # 샘플 템플릿 JSON (site/page/menu/body)
├── migrations/      # PostgreSQL 마이그레이션 (001-007)
├── docs/            # 문서
│   ├── USAGE_GUIDE.md
│   ├── ACCESS_GUIDE.md
│   ├── DASHBOARD_GUIDE.md
│   ├── STATUS.md
│   ├── openapi.yaml
│   └── plan/PHASE42_PLAN.md
├── loadtest/        # 시나리오 스크립트
├── monitoring/      # Prometheus/Grafana 설정
├── tests/           # 통합 테스트
├── docker-compose.yml
├── Dockerfile
├── .env.example
└── Cargo.toml
```

---

## 빠른 시작

### 1. Windows에서 직접 실행 (개발용)

```batch
C:\Users\USER\run_meta_server.bat
```

- MemoryStore 모드 (데이터 비영속)
- `http://localhost:3000` 접속

### 2. Docker (WSL) 실행 (운영용)

```bash
wsl -d Ubuntu -- docker compose up -d meta_server
```

- PostgreSQL + Redis 활성화
- `http://{컴퓨터 IP}:3000` 접속

### 3. 관리자 접속

| 항목 | 값 |
|---|---|
| URL | `http://localhost:3000/admin` |
| 토큰 | `admin-secret-123` |

### 4. API 테스트

```bash
# 헬스체크
curl http://localhost:3000/health

# 템플릿 목록
curl "http://localhost:3000/api/templates?tenant=default"

# 블로그 글 생성 (템플릿 기반)
curl -X POST "http://localhost:3000/api/blog/items?template_id=body.blog_post&sync_policy=live" \
  -H "Content-Type: application/json" \
  -d '{"title":"새 블로그 글"}'

# 블로그 글 목록
curl "http://localhost:3000/api/blog/articles"
```

---

## 핵심 기능

### 계층형 템플릿 시스템 (Phase 41/42)

```
site.shop (site)
├── page.home (page)
│   └── menu.main (menu)
│       └── menu.sub_categories (menu)
├── menu.main (menu)
│   └── menu.sub_products (menu)
├── body.blog_post (body, domain: blog)
├── body.hero (body, domain: board)
└── body.footer (body, domain: board)
```

- **Bump**: 템플릿 버전 +1 (기존 인스턴스 영향 없음)
- **Propagate**: `sync_policy=live` 인스턴스에 자동 전파
- **Provisioning**: 테넌트별 사이트 템플릿 복제

### 3단계 캐시

| 레벨 | 저장소 | 접근 속도 |
|---|---|---|
| L1 | 서버 인메모리 | 0ms |
| L2 | Redis | ~1ms |
| L3 | PostgreSQL | ~5-20ms |

- Redis Pub/Sub으로 L1 동적 무효화
- `fail-safe`: 새 스크립트 오류 시 직전 안정 버전으로 후퇴

### 다중 테넌트

- `X-Tenant-Id` 헤더로 테넌트 식별 (기본: `default`)
- Row-level 격리 (tenant_id 컬럼)
- 관리자 API는 테넌트 격리 우회

---

## 기술 스택

| 영역 | 기술 |
|---|---|
| 언어 | Rust (stabilized), JavaScript (rquickjs) |
| 웹 프레임워크 | Axum 0.7 |
| 비동기 런타임 | Tokio 1.53 |
| JS 엔진 | rquickjs 0.14 (QuickJS) |
| DB | PostgreSQL 15 (tokio-postgres) |
| 캐시 | Redis 7 (redis crate) |
| 인증 | JWT (jsonwebtoken 9) |
| 모니터링 | Prometheus + Grafana |
| 테스트 | cargo test (138개) |

---

## 에러 코드 규칙

`[도메인 1자][레이어 1자][번호 3자]`

| 도메인 | 설명 |
|---|---|
| `C` | Core / Infrastructure |
| `S` | Shop / 쇼핑몰 |
| `E` | ERP |
| `B` | Blog |

| 레이어 | 설명 |
|---|---|
| `R` | Rust Core |
| `J` | JavaScript |

예: `CJ101` (JS 스크립트 구문 오류), `CR001` (메모리 부족), `SJ202` (할인율 검증 위반)

---

## 문서

| 문서 | 내용 |
|---|---|
| `docs/USAGE_GUIDE.md` | 전체 사용 가이드 |
| `docs/ACCESS_GUIDE.md` | 접속 정보, 계정, API |
| `docs/DASHBOARD_GUIDE.md` | 대시보드 화면별 가이드 |
| `docs/STATUS.md` | Phase별 진행 상태 |
| `docs/DEVELOPMENT_PLAN.md` | 개발 계획 |
| `docs/plan/PHASE42_PLAN.md` | Phase 42 (P1-P5) 계획 |
| `docs/openapi.yaml` | OpenAPI 스펙 |
| `docs/setting/HANDOVER_phase_41_template.md` | Phase 41 핸드오버 |
| `AGENTS.md` | AI 에이전트 개발 가이드라인 |

---

## 라이선스

내부 프로젝트. 저작권 (c) 2026 MetaServer.
