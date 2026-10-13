# HANDOVER: Phase 13 — Docker 빌드 실 검증 + 마이그레이션 자동 적용 + 모니터링

> 이 문서는 Phase 13을 완료하고 단절했을 때, 다음 개발자(또는 다음 세션의 AI 에이전트)가
> 이어서 작업을 진행할 수 있도록 작성된 핸드오버 문서입니다.

---

## 1. Phase 13 완료 내용

### 1.1 마이그레이션 자동 적용

`src/meta/migrate.rs` 신규 생성:

```rust
pub async fn run_migrations(client: &Client) -> Result<Vec<String>, DataStoreError> {
    let files = list_migration_files()?; // migrations/*.sql 오름차순
    let mut applied = Vec::new();
    for file in &files {
        let sql = std::fs::read_to_string(file)?;
        client.batch_execute(&sql).await?;
        applied.push(...);
    }
    Ok(applied)
}
```

`PostgresStore::connect()`에서 자동 호출:
- PostgreSQL 연결 성공 시 `migrations/001_init.sql` → `002_domain_tables.sql` 순차 적용
- `CREATE TABLE IF NOT EXISTS`로 멱등성 보장
- 실패 시 명시적 에러 반환 (기동 실패)

### 1.2 모니터링 엔드포인트 확장

`GET /health` (기존: `"ok"` → 신규: JSON):

```json
{
  "status": "ok",
  "engine": { "memory_limit": 67108864, "cpu_timeout_ms": 1500, "memory_usage": {...} },
  "database": { "backend": "postgres", "migrations_applied": ["001_init.sql", "002_domain_tables.sql"] },
  "redis": { "enabled": true },
  "pipelines": { "board": "initialized", "erp": "initialized", "blog": "initialized", "shop": "initialized" },
  "socket": { "active_sessions": 5 }
}
```

`build_router` 시그니처에 3개 인자 추가:
```rust
build_router(
    engine, board, domains, socket_config, socket_script,
    db_backend: String,      // NEW
    migrations: Vec<String>, // NEW
    redis_enabled: bool,     // NEW
)
```

### 1.3 SessionPool len()

`SessionPool`에 `AtomicUsize` 카운터 추가:
- `add_session()`: `fetch_add(1)`
- `remove_session()`: `fetch_sub(1)`
- `len()`: `load(SeqCst)` — sync 접근 (health 엔드포인트용)

### 1.4 Dockerfile 개선

```dockerfile
# HEALTHCHECK 추가
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:3000/health || exit 1
```

런타임 이미지에 `curl` 추가.

### 1.5 전체 테스트: **80개 전부 통과**

| 테스트 파일 | 개수 |
|-------------|------|
| Phase 13 (migrate) | 3 |
| 기존 (Phase 1~12) | 77 |
| **합계** | **80** |

---

## 2. 주의사항

### 2.1 build_router 시그니처

Phase 12: 5개 인자 → Phase 13: 8개 인자.

```rust
// Phase 13
build_router(
    engine: Engine,
    board: Option<Arc<BoardPipeline>>,
    domains: Vec<Arc<DomainPipeline>>,
    socket_config: SocketConfig,
    socket_script: String,
    db_backend: String,       // "postgres" | "memory"
    migrations: Vec<String>,  // ["001_init.sql", "002_domain_tables.sql"]
    redis_enabled: bool,
)
```

기존 테스트에서 `"memory".into(), vec![], false` 추가 필요.

### 2.2 /health 응답 형식 변경

기존: `"ok"` (plain text)
신규: JSON 객체 (위 예시 참조)

API 클라이언트는 `/health` 응답을 JSON으로 파싱해야 합니다.

### 2.3 마이그레이션 순서

`migrations/` 디렉토리의 `.sql` 파일이 파일명 오름차순으로 적용됩니다.
- `001_init.sql`: `meta_modules`, `board_posts`
- `002_domain_tables.sql`: `erp_purchase_orders`, `blog_articles`, `shop_orders`

새 마이그레이션을 추가할 때 `003_xxx.sql`과 같이 번호를 매깁니다.

### 2.4 Docker HEALTHCHECK

컨테이너 기동 후 5초(시작 대기) 후 30초 간격으로 `/health`를 호출합니다.
3회 연속 실패 시 컨테이너를 "unhealthy"로 표시합니다.

---

## 3. 다음 단계 (Phase 14)

### 3.1 Docker 실 검증 (WSL2)
- `docker build -t meta_server .` → 이미지 생성
- `docker run` + 실 PostgreSQL/Redis 연결
- `curl /health` → JSON 상태 확인
- API 요청 테스트 (board, erp, blog, shop)

### 3.2 Prometheus Metrics
- `/metrics` 엔드포인트 (Prometheus format)
- L1/L2/L3 캐시 적중률, 훅 실행 시간, 에러 카운트

### 3.3 인증/인가
- JWT 토큰 인증
- 도메인별 권한 체크

---

## 4. 빌드 및 테스트

```bash
# 전체 테스트 (80개)
cargo test

# Phase 13 테스트만
cargo test --test phase13_migrate

# Docker 빌드 (WSL2)
wsl -d Ubuntu -- docker build -t meta_server -f /mnt/c/ai/meta_server/Dockerfile /mnt/c/ai/meta_server
```
