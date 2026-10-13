# HANDOVER: Phase 41 Step 1 - 계층형 템플릿 (데이터 모델/채널/읽기 API)

## 1. 이 파일의 목적
Phase 41 Step 1을 완료한 뒤 **Step 2**(스태킹/인스턴스 생성/Q3)를 시작할 때 이 문서를 읽고 바로 이어가기 위한 핸드오버.

## 2. 완료된 범위 (Step 1)
- **데이터 모델**: `meta_templates` 트리 노드 + 기존 도메인 테이블에 `template_id` 컬럼.
- **두 채널**: 파일(`templates/*.json` 시드) + DB(`upsert_template`).
- **읽기 API**: 목록/상세/트리 결합 3개.
- **테스트**: 6개 (총 129개 통과).

## 3. 파일/위치
| 파일 | 내용 |
|---|---|
| `migrations/006_meta_templates.sql` | `meta_templates` + 3개 도메인 테이블에 `template_id` |
| `migrations/007_template_instance.sql` | 3개 도메인 테이블에 `template_version`/`sync_policy` |
| `src/meta/store.rs` | `MetaTemplate`/`TemplateTree` 모델 + `DataStore` 9 메서드 (5 읽기 + 4 Step 2) + MemoryStore/PostgresStore 구현 + `seed_templates_from_files()` + `row_to_template()` + `BoardPost`/`BlogArticle`/`ShopProduct` 템플릿 필드 |
| `src/meta/mod.rs` | 모델/시드 함수 공개 (`MetaTemplate`/`TemplateTree`/`seed_templates_from_files`) |
| `src/http.rs` | `TemplateQuery`/`TemplateInstanceQuery`/`SiteTemplateQuery` struct + 읽기 API 3개 + Step 2 API 4개 (board_instance_create/admin_template_bump/admin_template_propagate/admin_apply_site_template) + `collect_template_fields()` 헬퍼 + 라우트 7개 |
| `src/main.rs` | 시드 호출 (`seed_templates_from_files`) |
| `src/admin.rs` | 대시보드 "Templates (Hierarchical)" 섹션 (트리 뷰 + Bump/Propagate + Site Provisioning JS) |
| `templates/*.json` | 4개 샘플 템플릿 (site.shop/page.home/menu.main/body.blog_post) |
| `tests/phase41_template.rs` | 10개 테스트 (Step 1 6개 + Step 2 4개) |
| `tests/phase30_chaos.rs` | `FailingStore` 9개 템플릿 메서드 위임 추가 |
| `docs/openapi.yaml`/`.json` | 템플릿 엔드포인트 7개 문서화 |
| `docs/setting/PHASE41_DESIGN.md` | 전체 설계 (단1/단2) |

## 4. API 계약 (단1)
| 메서드 | 경로 | 쿼리 | 응답 |
|---|---|---|---|
| GET | `/api/templates` | `?tenant=&domain=&parent=` | `{code, count, templates:[...]}` |
| GET | `/api/templates/:id` | `?tenant=` | `{code, template:{...}}` |
| GET | `/api/templates/:id/tree` | `?tenant=` | `{code, template:{..., children:[...]}}` (재귀) |

에러 코드: `CR4006`(템플릿 미존재, 200+코드), `CR0005`(DB 오류, 200+코드).

## 5. 디자인 결정 (사용자 승인)
- **단1/단2 2단 분할**: 단1=데이터/채널/읽기, 단2=스태킹/인스턴스/Q3.
- **파일 형식**: JSON, 1개 파일당 템플릿 1개.
- **인스턴스 저장**: 기존 도메인 테이블 + `template_id` (단2에서 version/sync_policy 추가).
- **테넌트 추출**: `?tenant=` > `X-Tenant-Id` > `default`.
- **채널 우선순위**: DB(실행 시) > 파일(시드/기본값). 파일 시드는 멱등.

## 6. Phase 38 직렬화 준수 (중요)
- `payload` 컬럼은 **JSONB가 아닌 TEXT** (JSONB 바인딩 직렬화 특이점 회피).
- `parent_id`/`domain`/`file_path`는 `NOT NULL DEFAULT ''` → **COALESCE로 NULL-안정**하게 바인딩.
- `upsert_template`은 11개 파라미터 중 `updated_at`을 `now()` 디폴트로 대체 (f64 타임스탬프 바인딩 회피).
- `list_templates`는 동적 SQL+query_raw 대신 **필터 조합별 정적 쿼리 4개 분기** (query_raw 직렬화 회피).

## 7. 순환 참조 방지 (주의)
- `build_tree_inner`는 비동기 재귀 → `Box::pin(go(...))`로 future 크기 제한 (E0733).
- `(template_id, version)` HashSet으로 방문 노드 추적 → 사이클(A→B→A) 시 재진입 차단.
- 트리 `TemplateTree`는 `(template: MetaTemplate, children: Vec<TemplateTree>)`.

## 8. 완료된 범위 (Step 2)
Step 2는 인스턴스 생성 스태킹 + 개선 전파 + 사이트 템플릿 프로비저닝을 완료했다.

**Schema:**
- `migrations/007_template_instance.sql`: `board_posts`/`blog_articles`/`shop_products`에 `template_version`(int4)/`sync_policy`(text) 컬럼 추가.
- 모델(`src/meta/store.rs`): `BoardPost`/`BlogArticle`/`ShopProduct`에 `template_id`/`template_version`/`sync_policy` 필드 추가. `default_sync_policy()`="frozen".

**DataStore 신규 메서드 4개** (MemoryStore + PostgresStore):
- `apply_template_to_board_post(post_id, template_id, version, payload_fields, sync_policy)`: 게시글에 바인딩 기록 + extra에 payload 병합.
- `bump_template_version(id, tenant)`: 활성 버전 +1 (기존 인스턴스 스냅샷 보존).
- `propagate_template_to_board_posts(tenant, id, payload_fields)`: `sync_policy='live'` 인스턴스만 재적용, 영향 수 반환.
- `apply_site_template(tenant, site_template_id)`: site 트리에서 `body` 노드를 대상 테넌트로 복사.

**API 4개** (`src/http.rs`):
- `POST /api/board/items?template_id=&sync_policy=`: 템플릿 `payload.fields[].default` 수집 → 게시판 파이프라인 생성 → 바인딩 기록.
- `POST /api/admin/templates/:id/bump`.
- `POST /api/admin/templates/:id/propagate`.
- `POST /api/admin/tenants/:id/apply-site-template?site_template_id=`.

**Dashboard** (`src/admin.rs`): "Templates (Hierarchical)" 섹션 — 트리 뷰 + Bump/Propagate 버튼 + Site Provisioning.
**OpenAPI**: 템플릿 엔드포인트 7개 문서화 (Step 1 3개 + Step 2 4개).
**Tests**: `tests/phase41_template.rs` 4개 신규 테스트 (바인딩/live-only 전파/버전 증분/사이트 프로비저닝). 총 133개 통과.

## 9. 다음 액션 (Step 2 이후)
1. Docker 실 검증: Phase 38 직렬화 특이점이 Docker에만 발생하는지 확인 (Windows는 통과).
2. `POST /api/blog/articles`와 `POST /api/shop/products`에도 인스턴스 생성 스택킹 적용 (게시판에만 구현됨).
3. 대시보드: 템플릿 기반 인스턴스 생성 버튼 추가 (현재 읽기/전파만).
4. 다중 도메인 템플릿 체인 (site→page→body→menu→sub-menu) 실서버 시나리오 검증.

## 9. 주의/함정
- `POST /api/admin/...`는 Admin 인증(`X-Admin-Token`) 필요. `GET /api/templates/*`는 JWT 공개 API.
- `cargo test`는 **Windows MSVC** 환경에서 `vcvars64.bat` 소스 후 실행 (cl.exe C-컴파일러 필요). Docker가 없으면 이 방법이 유일.
- 서버 포트는 `src/main.rs`에서 **3000으로 하드코드**. 테스트 시 포트 충돌 주의.
- `tests/phase30_chaos.rs`의 `FailingStore`에 9개 템플릿 메서드 위임 추가됨 (Step 1 5개 + Step 2 4개, DataStore trait 확장 반영).
- `POST /api/board/items?template_id=`는 **게시판 파이프라인**을 경유한다 (훅 발동 + 저장). 템플릿 default는 `payload.fields[].default` 배열에서 수집되며, 요청 바디의 `title`/`body`가 있으면 덮어씀.
- `bump`는 **신규 버전 row 추가** (기존 버전 보존). `get_template`은 `DISTINCT ON`으로 활성(최신) 버전을 반환.
- `propagate`는 `extra` 컬럼에 `payload_fields` JSON을 **전체 대치**한다 (병합 아님). frozen 인스턴스는 영향 안 받음.
- `apply_site_template`은 site 트리에서 `ttype='body'` 노드만 복사 (site/page 노드는 복사 안 함). 대등 테넌트 버전은 1로 시작.
