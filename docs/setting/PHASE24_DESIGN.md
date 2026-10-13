# Phase 24 설계: 에러 로그 기록 + 성능 최적화

> 훅/시스템 에러를 `error_logs`에 기록하고, DB/Redis 연결 최적화를 한다.

---

## 1. 에러 로그 기록

### D24-1: HTTP 핸들러 레벨에서 에러 기록
`handle_domain`, `board_create`에서 에러 발생 시 `state.record_error()` 호출:
- CJ1101: 구문 오류
- CJ1102: CPU Timeout
- CJ1103: 메모리 초과
- CJ1104: 비즈니스 예외
- CR0001: 메모리 부족
- CR0005: 파이프라인 미기동

### D24-2: `AppState::record_error()` 메서드
```rust
impl AppState {
    pub fn record_error(&self, code: &str, message: &str, domain: &str) {
        let mut logs = self.error_logs.lock().unwrap_or_else(|e| e.into_inner());
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        logs.push_back(ErrorLogEntry {
            code: code.to_string(),
            message: message.to_string(),
            timestamp: now,
            domain: domain.to_string(),
        });
        if logs.len() > 100 {
            logs.pop_front();
        }
    }
}
```

### D24-3: `metrics.record_hook_error()` 연동
에러 발생 시 `metrics.record_hook_error()`도 함께 호출

---

## 2. 성능 최적화

### D24-4: DB 연결 풀링 상태 모니터링
- `/api/admin/status`에 `db_connection_pool` 정보 추가
- 현재: `sqlx`는 자체 연결 풀 관리 (기본 설정)
- 모니터링용: `pool_size`, `idle_connections` (가능한 범위에서)

### D24-5: Query 최적화
- `INSERT` 후 `RETURNING` 사용 (이미 구현)
- `SELECT` 시 필요한 컬럼만 조회 (이미 구현)
- 추가: `EXPLAIN` 기반 성능 분석 (미구현, Phase 25)

### D24-6: Cache Warming
- 서버 기동 시 자주 접근되는 모듈을 L1 캐시에 로딩
- `base` + `extension` 모듈 사전 로딩

---

## 3. 산출물

| 파일 | 변경 |
|------|------|
| `src/http.rs` | `record_error()` 메서드 + 에러 기록 호출 |
| `src/pipeline.rs` | `map_infra_error`에서 `record_error` 호출 |
| `src/domain.rs` | `map_infra_error`에서 `record_error` 호출 |
