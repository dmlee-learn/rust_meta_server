# Phase 23 설계: 대시보드 실시간 WebSocket + 에러 로그 + 서버 제어

> 대시보드를 실시간으로 만들고, 에러 로그를 표시하고, 서버 제어를 가능하게 한다.

---

## 1. 에러 로그 (Ring Buffer)

### D23-1: `AppState`에 `error_logs` 추가
- `std::sync::Mutex<VecDeque<ErrorLogEntry>>` (최대 100개)
- `record_error(code, message, timestamp)` 메서드
- 훅 에러, 시스템 에러 기록 시 호출

### D23-2: `GET /api/admin/logs`
- 마지막 50개 에러 로그 반환
- `[{code, message, timestamp, domain}]`

---

## 2. 서버 제어

### D23-3: `POST /api/admin/cache/flush`
- 모든 L1 캐시 (board + domains) flush
- `{ "code": "OK", "flushed": N }`

### D23-4: `GET /api/admin/ping`
- 서버 생존 확인 (health보다 가볍게)
- `{ "code": "OK", "ts": "unix:1234567890" }`

---

## 3. 실시간 WebSocket

### D23-5: `GET /ws/admin`
- 별도 WebSocket 엔드포인트 (admin 대시보드 전용)
- 5초 간격으로 metrics JSON push
- `ws://localhost:3000/ws/admin`
- JWT/Rate Limit 미들웨어 제외

### D23-6: 대시보드 JS 업데이트
- WebSocket 연결 시도 → 성공 시 5초 polling 중단
- WebSocket 실패 시 5초 polling 폴백
- 에러 로그 패널 추가
- 캐시 flush 버튼 추가

---

## 4. 파일 변경

| 파일 | 변경 |
|------|------|
| `src/admin.rs` | 대시보드 HTML 업데이트 (WS + 로그 + flush) |
| `src/http.rs` | `/api/admin/logs`, `/api/admin/cache/flush`, `/api/admin/ping`, `/ws/admin` |
| `src/socket/ws.rs` | `ws_admin_handler` 추가 |
| `src/lib.rs` | (변경 없음) |
