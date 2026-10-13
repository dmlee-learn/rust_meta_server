# Phase 19 설계: rquickjs 시그널 + User Rate Limit + WebSocket 확장

> rquickjs 별도 쓰레드 eval, user-based rate limit, WebSocket domain topic.

---

## 1. rquickjs C 라이브러리 시그널 문제 해결

### D19-1: 별도 쓰레드에서 JS eval
- `SandboxedContext::eval_string()`을 `tokio::task::spawn_blocking`에서 실행
- crash가 컨테이너 전체를 종료시키지 않도록 분리
- async 컨텍스트에서 sync blocking rquickjs 코드 실행

### D19-2: Docker 시그널 처리
- Docker `--restart=always` 유지
- rquickjs 0.14 C 라이브러리 시그널 처리 버그 확인

---

## 2. User-based Rate Limit

### D19-3: JWT sub 기반 rate limit
- JWT 인증 시 `Claims.sub`를 user identifier로 사용
- `RateLimiter::check_user()` — user별 요청 체크
- 환경변수: `RATE_LIMIT_USER_PER_MIN` (기본 60)

### D19-4: IP + User 조합
- IP 기반 global limit + User 기반 limit 모두 적용
- User limit 초과 시 429 CR4003 반환

---

## 3. WebSocket 확장

### D19-5: Domain별 topic
- WebSocket 연결 시 `?domain=erp` 쿼리 파라미터로 domain 지정
- domain별 브로드캐스트: 해당 domain의 유효화 이벤트만 수신
- `/socket` 엔드포인트에 `domain` 쿼리 파라미터 추가

### D19-6: Real-time 유효화 알림
- PostgreSQL/Redis 변경 시 WebSocket 클라이언트에 알림
- `meta:invalidation` 채널에서 수신 → WebSocket broadcast

---

## 4. 산출물

| 파일 | 내용 |
|------|------|
| `src/engine/runtime.rs` | `spawn_blocking` JS eval |
| `src/rate_limit.rs` | User-based rate limit |
| `src/socket.rs` | Domain topic + real-time 알림 |
| `src/http.rs` | WebSocket domain query |
| `tests/phase19_user_rate_limit.rs` | User rate limit 테스트 |
