# Phase 25 설계: 보안 강화

> Admin API 인증 + CORS 정책 + Input validation 강화.

---

## 1. Admin API 인증

### D25-1: `ADMIN_TOKEN` 환경변수
- `ADMIN_TOKEN` 환경변수 설정 시 `/api/admin/*`에 `X-Admin-Token` 헤더 요구
- 미설정 시 (개발 환경) 인증 없이 접근 가능 (기존 동작 유지)
- 대시보드 HTML에 token 입력란 추가

### D25-2: `admin_auth_middleware`
- `/api/admin/*` 경로에만 적용
- `ADMIN_TOKEN` 미설정 → pass through
- 설정된 경우 `X-Admin-Token` 헤더 비교
- 불일치 시 `401 {"code":"CR6001","message":"Unauthorized admin access"}`

### D25-3: 에러 코드
- `CR6001`: Admin 인증 실패

---

## 2. CORS 정책

### D25-4: `CORS_ORIGINS` 환경변수
- 쉼표 구분 origin 목록 (예: `http://localhost:3000,http://localhost:3001`)
- 미설정 시: `*` (전부 허용, 개발용)

### D25-5: CORS 미들웨어
- `Access-Control-Allow-Origin` 헤더 설정
- `Access-Control-Allow-Methods`: GET, POST, PUT, DELETE, OPTIONS
- `Access-Control-Allow-Headers`: Content-Type, Authorization, X-Admin-Token
- `Access-Control-Max-Age`: 86400

---

## 3. Input Validation 강화

### D25-6: Board Post Validation
- `title`: 1~200자, 빈 값 금지
- `body`: 0~10000자
- `extra`: object (선택)

### D25-7: ERP Purchase Order Validation
- `workflow_id`: 1~100자
- `supplier`: 1~200자
- `quantity`: 1~1000000 (음수 금지)
- `amount`: 0~999999999999 (음수 금지)

### D25-8: Blog Article Validation
- `title`: 1~200자
- `body`: 0~10000자
- `category`: 1~50자

### D25-9: Shop Order Validation
- `product_id`: 1~100자
- `quantity`: 1~1000000
- `price`: 0~999999999999

### D25-10: 에러 코드
- `CJ1201`: Input validation 실패 (필수 필드 누락, 길이 초과, 음수 등)

---

## 4. 파일 변경

| 파일 | 변경 |
|------|------|
| `src/auth.rs` | `admin_auth_middleware` + CORS 미들웨어 |
| `src/http.rs` | 미들웨어 적용, Input validation |
| `src/pipeline.rs` | Board input validation |
| `src/domain.rs` | Domain input validation |
| `docker-compose.yml` | `ADMIN_TOKEN` 환경변수 |
| `.env.example` | `ADMIN_TOKEN`, `CORS_ORIGINS` |
