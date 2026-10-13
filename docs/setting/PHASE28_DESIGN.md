# Phase 28 설계: API 문서화 (OpenAPI 업데이트 + Swagger UI)

> OpenAPI spec 최신화 + 서버 내장 Swagger UI.

---

## 1. OpenAPI spec 갱신

### D28-1: 신규 endpoint 추가
- `/admin` (GET): 대시보드 HTML
- `/api/admin/status` (GET): 서버 상태
- `/api/admin/metrics` (GET): 메트릭스 스냅샷
- `/api/admin/logs` (GET): 에러 로그
- `/api/admin/cache/flush` (POST): 캐시 flush
- `/api/admin/ping` (GET): ping
- `/api/admin/ping/trigger` (POST): 즉시 상태 확인
- `/api/admin/export` (GET): JSON export
- `/ws/admin` (GET): admin WebSocket
- `/metrics`, `/api/engine/info`, `/api/engine/eval`

### D28-2: 신규 에러 코드 추가
- `CR6001`: Admin 인증 실패 (401)
- `CJ1201`: Input validation 실패 (422)
- `CR4001/4002/4003`: Rate limiting
- `CR0005`: 파이프라인 미기동

### D28-3: Security
- `bearerAuth` (JWT) — 일반 API
- `adminAuth` (X-Admin-Token header) — admin API

### D28-4: 422 response
- board/erp/blog/shop create 요청에 `422` (CJ1201 input validation) 응답 추가

---

## 2. Swagger UI 서버 내장

### D28-5: `GET /swagger`
- swagger-ui-dist CDN으로 렌더링되는 HTML
- OpenAPI spec: `/docs/openapi.yaml` 또는 서빙된 JSON
- `GET /openapi.json`: spec JSON 서빙 (YAML→JSON 또는 그대로)

### D28-6: `GET /openapi.json`
- OpenAPI spec을 JSON으로 서빙
- 파일 read 후 서빙 (버전 업뎅 시 자동 반영)

---

## 3. 파일 변경

| 파일 | 변경 |
|------|------|
| `docs/openapi.yaml` | 신규 endpoint + 에러 코드 + security |
| `src/http.rs` | `GET /swagger`, `GET /openapi.json` 루트 + 핸들러 |
| `src/admin.rs` | Swagger UI HTML 템플릿 (선택) |

---

## 4. 검증

- `GET /openapi.json`: spec JSON 확인
- `GET /swagger`: Swagger UI 렌더링
- 전체 endpoint 문서화 확인
- 전체 테스트 통과
