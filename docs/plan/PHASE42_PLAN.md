# 다음 세션 작업 계획 (Post Phase 41 Step 2)

> 작성일: 2026-10-07
> 전제: Phase 41 Step 1/2 완료 (133개 테스트 통과, Windows MSVC 빌드 확인)

---

## 1. 시스템 현황 요약

Phase 41 Step 2까지 완료된 항목:

| 영역 | 완료 상태 |
|---|---|
| 템플릿 데이터 모델 | `meta_templates` 테이블 + `MetaTemplate`/`TemplateTree` 모델 |
| 파일/DB 듀얼 채널 | `templates/*.json`(4개 샘플) + `seed_templates_from_files()` |
| 읽기 API | `GET /api/templates`, `/api/templates/:id`, `/api/templates/:id/tree` |
| 인스턴스 생성 스태킹 | `POST /api/board/items?template_id=` |
| 개선 전파 (Q3) | `bump` + `propagate` + `sync_policy` |
| 사이트 프로비저닝 | `POST /api/admin/tenants/:id/apply-site-template` |
| Dashboard UI | 템플릿 트리 뷰 + Bump/Propagate + Site Provisioning |
| OpenAPI | 7개 템플릿 엔드포인트 |
| 테스트 | 133개 (0 실패) |

**미완료 항목:**
- Docker 실 검증 (Phase 38 직렬화 특이점)
- Blog/Shop 도메인 스태킹
- 대시보드 인스턴스 생성 버튼

---

## 2. 작업 우선순위

### P1: Docker 실 검증 (권장 최우선)

**왜 먼저 하는가:**
Phase 38에서 PostgreSQL 직렬화 특이점(JSONB/NUMERIC 바인딩, 2번째 파라미터화 쿼리)이
Windows MSVC에서는 통과하나 **Linux Docker에서 실패**하는 것이 확인되었다.
Phase 41 Step 2의 신규 SQL 쿼리(`apply_template_to_board_post`, `bump_template_version`,
`propagate_template_to_board_posts`, `apply_site_template`)도 같은 PostgreSQL 연결을
사용하므로 Docker 환경에서 동일 문제가 재현되는지 확인해야 한다.

**작업 항목:**
1. Dockerfile 빌드 확인: `cargo build --release` on Linux (musl/gnu)
2. Docker 내 PostgreSQL + Redis + 서버 기동
3. Phase 41 Step 2 신규 쿼리 실동작 확인:
   - `apply_template_to_board_post`: `extra` TEXT 컬럼에 JSON 서열화 후 UPDATE
   - `bump_template_version`: `meta_templates`에 신규 row INSERT
   - `propagate_template_to_board_posts`: 조건부 UPDATE (`sync_policy='live'`)
   - `apply_site_template`: 트리 DFS + `meta_templates`에 body 노드 INSERT
4. Phase 38 직렬화 회피 규칙 준수 여부 확인:
   - `&String` 바인딩은 TEXT 컬럼에만
   - `i64`/`i32` 네이티브 타입으로 numeric 바인딩
   - 2번째 파라미터화 쿼리 회피 (정적 쿼리 분기 사용)

**완료 기준:**
- Docker 내 `cargo test --release` 133개 통과
- 서버 기동 후 `POST /api/board/items?template_id=body.blog_post` 성공
- `POST /api/admin/templates/:id/bump` + `propagate` 성공

**예상 소요:** 2~4시간 (Docker 환경 셋업 포함)

---

### P2: Blog/Shop 도메인 스태킹 확장 ✅ 완료

**완료일:** 2026-10-08

**실현 항목:**
- [x] DataStore 5개 메서드: apply_template_to_blog_article / apply_template_to_shop_product / propagate_template_to_blog_articles / propagate_template_to_shop_products / propagate_template_to_all_domains
- [x] MemoryStore + PostgresStore 구현
- [x] API 2개: POST /api/blog/items, POST /api/shop/items
- [x] propagate에 domain=all|board|blog|shop 옵션
- [x] 대시보드 domain 선택 dropdown
- [x] OpenAPI 2개 엔드포인트 + domain 파라미터
- [x] 테스트 5개 (총 138개)

**작업 항목:**

1. **DataStore 메서드 추가:**
   - `apply_template_to_blog_article(article_id, template_id, version, payload, sync_policy)`
   - `apply_template_to_shop_product(product_id, template_id, version, payload, sync_policy)`
   - `propagate_template_to_blog_articles(tenant, id, payload)`
   - `propagate_template_to_shop_products(tenant, id, payload)`

2. **API 엔드포인트 추가:**
   - `POST /api/blog/articles?template_id=&sync_policy=`
   - `POST /api/shop/products?template_id=&sync_policy=`

3. **전파 API 확장:**
   - `propagate_template_to_board_posts` → `propagate_template_to_all_domains` (board+blog+shop)

4. **테스트:**
   - `tests/phase41_template.rs`에 blog/shop 스태킹 테스트 추가
   - `tests/phase30_chaos.rs` FailingStore에 신규 메서드 위임 추가

5. **Dashboard:**
   - 템플릿 선택 시 도메인(board/blog/shop) 선택 버튼 추가

**완료 기준:**
- 3개 도메인 모두 템플릿 기반 인스턴스 생성 가능
- `propagate`가 3개 도메인 모두에 적용
- 테스트 140개 이상 통과

**예상 소요:** 3~5시간

---

### P3: 대시보드 인스턴스 생성 UI ✅ 완료

**완료일:** 2026-10-12

**실현 항목:**
- [x] 인스턴스 생성 섹션 HTML (템플릿 dropdown, 도메인/sync_policy 선택, title 입력, 생성 버튼)
- [x] `createInstance()` JS 함수 (도메인별 API 호출)
- [x] `renderTemplates()`에서 body 템플릿 dropdown 자동 갱신
- [x] Windows MSVC 빌드 확인

**작업 항목:**

1. **인스턴스 생성 폼 추가:**
   - 템플릿 ID 선택 (dropdown: 목록 API에서 로드)
   - 도메인 선택 (board/blog/shop)
   - `sync_policy` 선택 (frozen/live)
   - 옵션: title/body/extra 커스텀 값 (템플릿 default 덮어쓰기)

2. **생성 후 피드백:**
   - 생성된 인스턴스 ID 표시
   - 템플릿 바인딩 정보 (template_id/version) 표시

3. **인스턴스 목록 뷰:**
   - 템플릿 ID로 인스턴스 목록 조회
   - 각 인스턴스의 `template_id`/`template_version`/`sync_policy` 표시

**완료 기준:**
- 대시보드에서 템플릿 선택 → 인스턴스 생성 → 결과 확인
- 인스턴스 목록에서 템플릿 바인딩 정보 확인

**예상 소요:** 2~3시간

---

### P4: 다중 도메인 체인 실서버 검증 ✅ 완료

**완료일:** 2026-10-12

**실현 항목:**
- [x] 4개 신규 샘플 템플릿 (body.hero, body.footer, menu.sub_products, menu.sub_categories)
- [x] `loadtest/phase42_scenario.bat`: 6단계 시나리오 스크립트
- [x] 실서버 검증: 8개 템플릿 시드, 트리 8개 노드, 인스턴스 생성, bump, propagate, 프로비저닝 전부 OK
- [x] `apply_site_template` 소스 테넌트 수정 ("site" → "default")

**작업 항목:**

1. **샘플 템플릿 확장:**
   - `site.shop` → `page.home` → `body.hero` + `menu.main` + `body.footer`
   - `menu.main` → `menu.sub_products` + `menu.sub_categories`
   - 총 8개 템플릿 (site 1 + page 1 + body 3 + menu 3)

2. **파일 시드 확장:**
   - `templates/`에 4개 신규 JSON 추가

3. **실서버 시나리오:**
   - 서버 기동 → 파일 시드 → 트리 API로 8개 노드 확인
   - `body.hero`로 인스턴스 생성 (게시판)
   - `menu.main`을 `live` sync로 바인딩 → `bump` + `propagate` → 인스턴스 업데이트 확인
   - `apply_site_template`으로 새 테넌트에 site 트리 프로비저닝

4. **시나리오 스크립트:**
   - `loadtest/phase41_scenario.sh`: 전체 시나리오 자동 실행 (curl 기반)

**완료 기준:**
- 트리 API가 8개 노드(3단계)를 올바르게 결합
- 인스턴스 생성 + 전파 + 프로비저닝 전체 플로우 동작
- 시나리오 스크립트 1회 실행으로 전체 검증

**예상 소요:** 1~2시간

---

### P5: Redis 캐시 연동 ✅ 완료

**완료일:** 2026-10-12

**실현 항목:**
- [x] `src/cache.rs`: `template_cache_get/set/invalidate/publish_invalidation` 4개 메서드
- [x] `template_list`/`template_get`/`template_tree` 핸들러에 Redis 캐시 경로
- [x] `admin_template_bump`에 캐시 무효화 (detail + tree + Pub/Sub)
- [x] `REDIS_ENABLED=false` 시 기존 DB 직접 경로 (무영향)
**예상 소요:** 2~3시간

---

## 3. 의존성 맵

```
P1 (Docker 검증)
    ↓ (Docker 환경 정상 확인 후)
P2 (Blog/Shop 스태킹)
    ↓
P3 (대시보드 인스턴스 UI)
    ↓
P4 (다중 도메인 체인 검증)
    ↓
P5 (Redis 캐시 연동)  [선택]
```

**병렬 가능:** P2와 P3는 병렬 가능 (서로 다른 파일 중심).
**순서 제약:** P1은 선행 (Docker 환경 미확인 시 P2~P4는 Windows만 가능).

---

## 4. 리스크

| 리스크 | 영향 | 완화 |
|---|---|---|
| Docker에서 Phase 38 직렬화 문제 재현 | P2~P4 진행 불가 | P1에서 먼저 확인, 필요 시 쿼리 수정 |
| `propagate`가 대량 인스턴스에서 성능 저하 | P2 | `WHERE template_id=$1 AND sync_policy='live'` 인덱스 확인 |
| `apply_site_template`의 DFS가 깊이 큰 트리에서 스택 오버플로 | P4 | 깊이 제한 또는 iteratif 변환 |
| 대시보드 JS에서 템플릿 트리 렌더링 성능 (대량 노드) | P3 | 노드 수 제한 또는 가상 스크롤 |

---

## 5. 다음 세션 시작 체크리스트

```
[ ] 1. Docker 환경 기동 (postgres + redis + server)
[ ] 2. P1: Docker 실 검증 — Phase 41 Step 2 쿼리 실동작 확인
[x] 3. P2 완료: Blog/Shop 스태킹 확장 (2026-10-08)
[x] 4. P3 완료: 대시보드 인스턴스 생성 UI (2026-10-12)
[x] 5. P4 완료: 다중 도메인 체인 실서버 검증 (2026-10-12)
[x] 6. P5 완료: Redis 캐시 연동 (2026-10-12)
[ ] 7. 각 Phase 완료 후 docs/STATUS.md + HANDOVER 갱신
```
