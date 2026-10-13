# Phase 13 설계: Docker 빌드 실 검증 + 마이그레이션 자동 적용 + 모니터링

> Docker 빌드 후 실 PostgreSQL/Redis와 연결하고,
> 마이그레이션을 자동 적용하며, 모니터링 엔드포인트를 확장한다.

---

## 1. 설계 결정

### D1: 마이그레이션 자동 적용

**결정: PostgreSQL 연결 시 `migrations/*.sql` 순차 적용**

```rust
// src/meta/migrate.rs
pub async fn run_migrations(client: &Client) -> Result<Vec<String>, DataStoreError> {
    let dir = std::path::Path::new("migrations");
    let files: Vec<_> = std::fs::read_dir(dir)
        .map_err(...)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map_or(false, |ext| ext == "sql"))
        .collect();
    
    files.sort(); // 001, 002 순서
    let applied = Vec::new();
    for file in files {
        let sql = std::fs::read_to_string(&file)?;
        client.batch_execute(&sql).await?;
        applied.push(file.file_name().to_string_lossy().to_string());
    }
    Ok(applied)
}
```

`PostgresStore::connect()`에서 호출:
```rust
pub async fn connect(config: &MetaStoreConfig) -> Result<Self, DataStoreError> {
    let (client, conn) = ...;
    run_migrations(&client).await?; // 마이그레이션 자동 적용
    Ok(Self { client: Arc::new(client) })
}
```

### D2: 모니터링 엔드포인트 확장

**결정: `GET /api/health`에 도메인별 상태 + DB/Redis 연결 상태 추가**

```json
{
  "status": "ok",
  "timestamp": "2026-10-02T00:00:00Z",
  "engine": {
    "memory_limit": 67108864,
    "memory_usage": { "malloc_limit": ..., "malloc_size": ... }
  },
  "database": {
    "backend": "postgres",
    "connected": true,
    "migrations_applied": ["001_init.sql", "002_domain_tables.sql"]
  },
  "redis": {
    "enabled": true,
    "connected": true
  },
  "pipelines": {
    "board": "initialized",
    "erp": "initialized",
    "blog": "initialized",
    "shop": "initialized"
  },
  "socket": {
    "max_sessions": 1000,
    "active_sessions": 5
  }
}
```

### D3: Dockerfile 개선

기존 Dockerfile에:
- `migrations/` 폴더 복사
- `ENTRYPOINT` 스크립트로 마이그레이션 확인
- 헬스체크 (`HEALTHCHECK`) 추가

### D4: WSL2 Docker 실 검증

- `docker build` → 이미지 생성
- `docker run` → 컨테이너 기동
- 실 PostgreSQL + Redis (기존 docker-compose)와 연결
- `curl /api/health` → 전체 상태 확인
- API 요청 테스트 (board, erp, blog, shop)

---

## 2. 산출물

| 파일 | 내용 |
|------|------|
| `src/meta/migrate.rs` | 마이그레이션 자동 적용 로직 |
| `src/meta/mod.rs` | `migrate` 모듈 등록 |
| `src/meta/store.rs` | `PostgresStore::connect()`에서 마이그레이션 호출 |
| `src/http.rs` | `/api/health` 확장 (도메인별 상태 + DB/Redis) |
| `Dockerfile` | HEALTHCHECK + ENTRYPOINT |
| `tests/phase13_migrate.rs` | 마이그레이션 + 모니터링 테스트 |

---

## 3. 주의사항

1. **마이그레이션 순서**: 파일명 오름차순 (001, 002, ...)
2. **멱등성**: `CREATE TABLE IF NOT EXISTS`로 재실행 안전
3. **에러**: 마이그레이션 실패 시 기동 실패 (명시적 에러)
4. **Docker HEALTHCHECK**: `curl -f http://localhost:3000/health`
