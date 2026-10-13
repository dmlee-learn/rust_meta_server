# Phase 27 설계: 대시보드 강화

> 히트맵, 서버 제어, 브라우저 알림, Export 기능.

---

## 1. 추가 API

### D27-1: `GET /api/admin/export`
- 현재 metrics + status + error logs를 단일 JSON로 반환
- `Content-Disposition: attachment; filename=meta_server_metrics_YYYYMMDD.json`
- 대시보드 Export 버튼에서 사용

### D27-2: `POST /api/admin/ping/trigger`
- 서버 상태를 즉각 확인 (health check trigger)
- DB + Redis 응답 확인 후 `{status, db_latency_ms, redis_latency_ms}` 반환
- 대시보드 "Ping Now" 버튼에서 사용

---

## 2. 대시보드 UI 추가

### D27-3: 히트맵 (도메인별 요청 분포)
- HTTP 요청 차트 아래에 도메인별 요청 수 히트맵 표시
- board/erp/blog/shop 각 칸의 밝기로 요청 수 표현
- 기존 `httpChart` 데이터 재사용

### D27-4: 서버 제어 버튼
- "🔄 Ping Now": `/api/admin/ping/trigger` 호출 후 지연 시간 표시
- "📊 Export JSON": `/api/admin/export` 호출 후 JSON 다운로드
- "🔔 Enable Alerts": Notification API 권한 요청 + 알림 활성화 토글

### D27-5: 브라우저 알림
- 에러 발생 시 (hook errors 증가) `Notification` API로 브라우저 알림
- 권한 허용 시에만 동작
- 대시보드 열려 있어야 함

---

## 3. 파일 변경

| 파일 | 변경 |
|------|------|
| `src/http.rs` | `GET /api/admin/export`, `POST /api/admin/ping/trigger` |
| `src/admin.rs` | 히트맵, 제어 버튼, 알림 로직 |
| `src/metrics.rs` | (필요 시) snapshot 구조 확인 |

---

## 4. 검증

- `GET /api/admin/export`: JSON 다운로드 확인
- `POST /api/admin/ping/trigger`: DB/Redis latency 반환
- 대시보드에서 히트맵, 버튼, 알림 동작 확인
- 전체 테스트 통과
