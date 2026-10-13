# HANDOVER: Phase 27 — 대시보드 강화

> 이 문서는 Phase 27를 완료하고 단절했을 때, 다음 개발자(또는 다음 세션의 AI 에이전트)가
> 이어서 작업을 진행할 수 있도록 작성된 핸드오버 문서입니다.

---

## 1. Phase 27 완료 내용

### 1.1 추가 API
- `GET /api/admin/export`: metrics + status + error logs 단일 JSON (attachment download)
  - `Content-Disposition: attachment; filename=meta_server_metrics_YYYYMMDD_HHMMSS.json`
  - keys: `exported_at`, `status`, `metrics`, `error_logs`
- `POST /api/admin/ping/trigger`: DB backend + Redis 상태 즉시 확인
  - `{code, status, db:{backend,ok,latency_ms}, redis:{enabled,ok}, ts}`

### 1.2 대시보드 UI
- **히트맵**: 도메인별 요청 수를 밝기로 표현 (board/erp/blog/shop 4칸)
- **서버 제어 버튼**:
  - "🔄 Ping Now": `/api/admin/ping/trigger` 호출
  - "📊 Export JSON": `/api/admin/export` 호출 후 JSON 다운로드
  - "🔔 Enable Alerts": Notification API 권한 + 알림 활성화 토글
- **브라우저 알림**: hook errors 증가 시 Notification API

### 1.3 Docker 실 검증
- `POST /api/admin/ping/trigger`: `{"db":{"backend":"postgres","ok":true},"redis":{"enabled":true,"ok":true}}`
- `GET /api/admin/export`: attachment + JSON keys 정상
- 대시보드: 히트맵/제어 버튼 렌더링 확인
- 컨테이너 healthy

### 1.4 테스트
- 전체 96개 테스트 통과

---

## 2. 주의사항

### 2.1 Admin Token
- Export/Ping trigger API도 `/api/admin/*`에 포함 → `X-Admin-Token` 필요
- 대시보드에서 fetch 호출 시 토큰 헤더 필요 (현행: 미포함)
- 개선: 대시보드에 admin token 입력란 추가 또는 세션 기반

### 2.2 브라우저 알림
- 대시보드 열려 있어야 함 (Notification API는 백그라운드 불가)
- 권한 허용 시에만 동작
- HTTPS 또는 localhost에서만 동작

### 2.3 히트맵
- `http_requests` 카운터 기반 (누적값)
- restart 시 초기화

---

## 3. 다음 단계 (Phase 28)

### 3.1 API 문서화 + Swagger
- OpenAPI spec 업데이트 (CJ1201, CR6001, CR4001~4003 추가)
- Swagger UI 서버 내장 (GET /swagger)
- API 사용 예시 (cURL, JavaScript, Python)
- 에러 코드 전체 목록 문서

### 3.2 Integration Test Suite
- E2E 테스트 (Docker Compose 기반)
- DB 상태 검증 (INSERT 후 SELECT 확인)
- Cache invalidation 테스트
- WebSocket 실시간 이벤트 테스트
- Rate limiting 테스트 (429 응답 확인)

---

## 4. 빌드 및 테스트

```bash
# 전체 테스트 (96개)
cargo test

# Docker Compose 기동
cd /mnt/c/ai/meta_server
docker compose up -d

# Ping trigger
curl -X POST http://localhost:3000/api/admin/ping/trigger \
  -H "X-Admin-Token: admin-secret-123"

# Export
curl http://localhost:3000/api/admin/export \
  -H "X-Admin-Token: admin-secret-123" -o metrics.json

# 대시보드 확인
# http://localhost:3000/admin

# Docker Compose 종료
docker compose down
```
