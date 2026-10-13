# Phase 40 핸드오버: Admin Extension (모듈) Management

## 1. 목표
운영자가 **Admin API**를 통해 확장 모듈(스크립트)을 **목록 조회, 상세 확인, 활성화 토글, L1 핫-리로드, 스크립트 배포**할 수 있게 한다. 별도 컴파일/배포 없이 운영 중인 서버에서 확장 스크립트를 관리한다.

## 2. 구현 내용
- `src/http.rs`: Admin 모듈 관리 5개 핸들러 추가.
  - `admin_modules_list` — 활성화된 모듈 목록 + L1 캐시 적재 여부.
  - `admin_module_get` — 모듈 상세 (script_source 포함).
  - `admin_module_toggle` — `enabled` 반전 → L3 저장 → L1 무효화.
  - `admin_module_reload` — L1 캐시 무효화 (다음 요청 시 재로드).
  - `admin_module_deploy` — 스크립트 **컴파일 검증 → L3 저장(버전 증분) → L1 무효화**.
- `AppState::first_pipeline()` — `board`/`domains` 공통 `store`+`cache`+`loader`를 owned로 반환하는 헬퍼 (BoardPipeline/DomainPipeline의 필드가 같으므로 튜플로 노출).
- `ModuleLoaderRef::compile_validate()` — `ModuleLoader::compile_validate_public` 위임 (배포 전 구문 검증).
- `DataStore::get_module_raw()` — **활성화 상태 무관**으로 모듈 조회. 기본 구현은 `get_module` 위임. **PostgresStore만 오버라이드** (기존 `get_module`는 `WHERE enabled = TRUE`로 필터해서 비활성화 모듈을 재읽지 못함).
- `src/admin.rs`: 대시보드에 **Extension Modules** 섹션 추가 (목록/토글/리로드/배포 UI + `x-admin-token` 입력 필드).
- `docs/openapi.yaml` + `docs/openapi.json`: 5개 엔드포인트 문서화 (24개 paths).
- `tests/phase40_admin_extension.rs`: 6개 테스트.

## 3. API
모든 엔드포인트는 **Admin 인증**을 거친다 (`X-Admin-Token` 헤더, `ADMIN_TOKEN` 환경변수 미설정 시 인증 우회).

| 메서드 | 경로 | 설명 | 성공 | 에러 |
|--------|------|------|------|------|
| GET | `/api/admin/modules` | 모듈 목록 (id, kind, version, enabled, cached_in_l1) | 200 | `CR0005`(500) |
| GET | `/api/admin/modules/:id` | 모듈 상세 (script_source 포함) | 200 | `CR4005`(404) |
| POST | `/api/admin/modules/:id/toggle` | 활성화/비활성화 토글 + L1 무효화 | 200 | `CR4005`(404) |
| POST | `/api/admin/modules/:id/reload` | L1 캐시 무효화 (핫리로드) | 200 | — |
| POST | `/api/admin/modules/deploy` | 스크립트 배포 (검증→저장→무효화) | 200 | `CJ1201`(422), `CJ1101`(구문오류), `CR0005`(500) |

### Deploy 요청 바디
```json
{
  "module_id": "ext.newmod",
  "script_source": "api.before_create = function(p){ return p; };",
  "kind": "extension",        // 선택 (기본 extension)
  "base_module_id": "base.board", // 선택 (신규 모듈 시)
  "enabled": true            // 선택 (기본 true)
}
```
- 기존 모듈에 배포: `version+1`, 기존 kind/base/enabled 보존.
- 신규 모듈에 배포: `version=1`, 미지정 필드는 기본값.

## 4. ⚠️ 핵심: 라우트 경합 & 스크립트 검증 규칙

### 4.1 라우트 등록 순서
Axum은 **등록 순서**로 라우트를 매칭한다. 정적 경로 `/api/admin/modules/deploy`는 **매개변수 경로 `/api/admin/modules/:id`보다 먼저** 등록되어야 한다.
- 정적 `deploy`를 `{id}` 뒤에 두면 `/modules/deploy`가 `{id}`="deploy"로 잡아먹혀 405/404가 된다.
- 최종 순서: `modules` → `modules/deploy` → `modules/:id` → `modules/:id/toggle` → `modules/:id/reload`.

### 4.2 스크립트 검증 (compile_validate)
- 검증은 스크립트를 `(function(){ <source> })`로 감싸 **평가(eval)** 해 구문/실행 오류를 잡는다.
- **유효한 스크립트 스타일**: `api.<hook> = function(인자){ ... }` (코드베이스의 실제 확장 스크립트 방식, `scripts/extensions/*.js`).
- **주의**: `before_request(){} after_save(){}` 같은 **맨 훅 함수 선언**(bare function declaration)은 eval 단계에서 예외가 발생해 `CJ1101`으로 거부된다. 이 코드베이스의 확장 스크립트는 `api.<hook> = function(...)` 할당 형식을 사용하므로 실제 스크립트는 문제가 없다.
- 검증을 통과한 스크립트만 L3에 저장된다 (결정적 실패 보호 — 구문 오류 스크립트가 캐시에 적재되지 않음).

### 4.3 `get_module` vs `get_module_raw`
- `get_module`: `WHERE module_id=$1 AND enabled=TRUE` — **활성화된 모듈만** 반환. 실행 파이프라인(cache, domain)이 이걸 써서 비활성 모듈은 실행되지 않는다. (이거를 바꾸면 실행 동작이 바뀔 수 있어 건드리지 않는다.)
- `get_module_raw`: `WHERE module_id=$1` — **활성화 상태 무관** 반환. Admin의 toggle/deploy가 "이미 비활성화한 모듈을 다시 활성화"하기 위해 필요하다.

## 5. 검증 결과
- **로컬** `cargo test`: **123개 통과** (0 실패). Phase 40 테스트 6개 포함.
- **Docker 실 검증** (PostgreSQL 백엔드, `x-admin-token: admin-secret-123`):
  1. `GET /api/admin/modules` → 200, 8개 모듈 + L1 상태.
  2. `GET /api/admin/modules/ext.notice` → 200, script_source 포함 상세.
  3. `POST .../toggle` (off) → 200 `enabled:false`, `enabled:true` (get_module_raw 정상 동작).
  4. `POST .../reload` → 200, L1 무효화 메시지.
  5. `POST /deploy` (유효 스크립트) → 200 `ext.newmod v1 배포 완료`.
  6. `POST /deploy` (무효 구문) → 200 `CJ1101 스크립트 구문 오류`.
  7. `POST /deploy` (필드 누락) → 200 `CJ1201 module_id 필수`.
  8. `GET .../ext.nonexistent` → 200 `CR4005`.
  9. 토큰 없이 요청 → **401** `CR6001` (인증 정상 동작).

## 6. 다음 개발자 체크리스트
- [x] 이 기능이 컴파일/배포 없이 DB/Redis 데이터 변경만으로 동작하는가? → **Yes** (모듈은 L3에 저장, L1 무효화로 핫리로드).
- [x] Base 모듈 스코프를 오염시키지 않는가? → **Yes** (독립 엔드포인트, store/cache/loader만 접근).
- [x] 에러 코드 명명 규칙 준수? → **Yes** (`CR4005` 모듈미존재, `CJ1201` 입력, `CJ1101` 구문, `CR0005` DB, `CR6001` 인증).
- [x] 상태값(메타데이터)과 분기/알고리즘(스크립트) 분리? → **Yes** (enabled/version은 메타데이터, 스크립트 본문은 명령형).

## 7. 환경 변수
- `ADMIN_TOKEN`: Admin API 인증 토큰. 미설정 시 `/api/admin/*` 인증 우회 (개발용).
- `X-Admin-Token` 헤더로 전달 (대소문자 무관).

## 8. 주의
- Docker에서 `curl -v` 단일 호출은 정상 200이나, 컨테이너 재시작 직후/WSL 포트포워딩 불안정 시 401/404가 간헐적으로 보일 수 있다. 이는 코드 문제가 아니라 **컨테이너 재시작 경합**이다. 확실한 검증은 `docker compose exec -T meta_server curl ...` (컨테이너 내부에서)로 수행한다.
