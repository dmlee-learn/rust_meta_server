# HANDOVER: Phase 28 — API 문서화 (OpenAPI + Swagger UI)

> 이 문서는 Phase 28를 완료하고 단절했을 때, 다음 개발자(또는 다음 세션의 AI 에이전트)가
> 이어서 작업을 진행할 수 있도록 작성된 핸드오버 문서입니다.

---

## 1. Phase 28 완료 내용

### 1.1 OpenAPI spec 갱신
- `docs/openapi.yaml`: 19개 endpoint
  - 기존: /health, /metrics, /api/board, /api/board/{id}, /api/erp, /api/blog, /api/shop, /ws
  - 신규: /admin, /api/admin/status|metrics|logs|cache/flush|ping|ping/trigger|export, /ws/admin, /openapi.json, /swagger
- `securitySchemes`: `bearerAuth` (JWT), `adminAuth` (X-Admin-Token)
- Create 요청에 `422` (CJ1201 input validation) 응답 추가

### 1.2 Swagger UI 서버 내장
- `GET /openapi.json`: `docs/openapi.json` 서빙 (YAML→JSON 변환)
- `GET /swagger`: swagger-ui-dist CDN 기반 UI

### 1.3 Docker
- `Dockerfile`: `COPY docs/ /app/docs/`
- `.dockerignore`: `docs/` 제외 제거

### 1.4 Docker 실 검증
- `/openapi.json`: 19 endpoint, securitySchemes 정상
- `/swagger`: UI 렌더링
- 컨테이너 healthy

### 1.5 테스트
- 전체 96개 테스트 통과

---

## 2. 주의사항

### 2.1 OpenAPI sync
- `docs/openapi.yaml` 수정 시 `docs/openapi.json`도 재생성 필요
- 변환 명령:
  ```bash
  python3 -c "import yaml,json; json.dump(yaml.safe_load(open('docs/openapi.yaml')), open('docs/openapi.json','w'), indent=2, ensure_ascii=False)"
  ```
- CI에서 자동 변환 스텝 추가 권장

### 2.2 Swagger UI
- CDN 의존 (외부 인터넷 필요)
- 오프라인 환경: swagger-ui-dist 로컬 서빙 필요

### 2.3 Admin API 문서
- `X-Admin-Token` 헤더 요구 (ADMIN_TOKEN 환경변수 설정 시)

---

## 3. 다음 단계 (Phase 29)

### 3.1 Integration Test Suite
- E2E 테스트 (Docker Compose 기반)
- DB 상태 검증 (INSERT 후 SELECT 확인)
- Cache invalidation 테스트
- WebSocket 실시간 이벤트 테스트
- Rate limiting 테스트 (429 응답 확인)

### 3.2 Load Testing
- Locust/k6 기반 부하 테스트
- Concurrent users 시나리오
- Throughput + error rate 측정

---

## 4. 빌드 및 테스트

```bash
# 전체 테스트 (96개)
cargo test

# OpenAPI JSON 재생성
python3 -c "import yaml,json; json.dump(yaml.safe_load(open('docs/openapi.yaml')), open('docs/openapi.json','w'), indent=2, ensure_ascii=False)"

# Docker Compose 기동
cd /mnt/c/ai/meta_server
docker compose up -d

# OpenAPI 확인
curl http://localhost:3000/openapi.json | python3 -m json.tool

# Swagger UI
# http://localhost:3000/swagger

# Docker Compose 종료
docker compose down
```
