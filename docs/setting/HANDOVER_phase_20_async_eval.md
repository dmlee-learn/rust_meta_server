# HANDOVER: Phase 20 — Async Eval + Docker 실 검증

> 이 문서는 Phase 20을 완료하고 단절했을 때, 다음 개발자(또는 다음 세션의 AI 에이전트)가
> 이어서 작업을 진행할 수 있도록 작성된 핸드오버 문서입니다.

---

## 1. Phase 20 완료 내용

### 1.1 Async Eval
- `with_observer_eval()` async fn 변경
- `sb.eval_string_observed(script)` 호출에 `.await` 추가
- rquickjs C 라이브러리 시그널 문제 대응 (spawn_blocking 대신 sync 호출 유지)

### 1.2 Docker Compose 실 검증
- 3개 컨테이너 모두 healthy
- `POST /api/board` → `{"code":"OK","id":1,"title":"Phase 20 Test"}`
- `POST /api/erp/purchase-orders` → `{"code":"OK","id":1,"domain":"erp"}`
- `POST /api/blog/articles` → `{"code":"OK","id":1,"domain":"blog"}`
- PostgreSQL INSERT 확인: board + erp
- 컨테이너 crash 없음

### 1.3 테스트
- 전체 96개 테스트 통과

---

## 2. 주의사항

### 2.1 Async Eval
`with_observer_eval()`이 Phase 20부터 async fn입니다.
호출 시 `.await`가 필요합니다:

```rust
let result = self.with_observer_eval(&sb, &script).await?;
```

### 2.2 rquickjs 시그널 문제
spawn_blocking 대신 sync 호출을 유지합니다.
`SandboxedContext`가 `Clone`을 지원하지 않기 때문입니다.
rquickjs 업그레이드 후 `Clone` 지원 시 spawn_blocking으로 전환 가능합니다.

### 2.3 Docker Compose
Phase 20부터 Docker Compose에서 board/erp/blog 전부 정상 작동합니다.
CJ1104 에러 시에도 컨테이너가 crash하지 않습니다.

---

## 3. 다음 단계 (Phase 21)

### 3.1 WebSocket 확장
- domain별 topic
- real-time 유효화 알림

### 3.2 Monitoring Dashboard
- Grafana dashboard
- Prometheus scraping 설정

### 3.3 API Documentation
- OpenAPI/Swagger spec
- API test suite

### 3.4 Performance Optimization
- Connection pooling
- Query optimization
- Cache warming

---

## 4. 빌드 및 테스트

```bash
# 전체 테스트 (96개)
cargo test

# Docker Compose 기동
cd /mnt/c/ai/meta_server
docker compose up -d

# API 호출
curl -X POST http://localhost:3000/api/board \
  -H "Content-Type: application/json" \
  -d '{"title":"Test","body":"Body"}'

curl -X POST http://localhost:3000/api/erp/purchase-orders \
  -H "Content-Type: application/json" \
  -d '{"workflow_id":"wf-1","supplier":"Acme","quantity":10}'

curl -X POST http://localhost:3000/api/blog/articles \
  -H "Content-Type: application/json" \
  -d '{"title":"Test","body":"Body","category":"tech"}'

# PostgreSQL 데이터 확인
docker exec meta_server-postgres-1 psql -U meta -d meta -c "SELECT * FROM board_posts;"

# Docker Compose 종료
docker compose down
```
