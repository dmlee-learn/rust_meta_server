# Phase 22 설계 (수정): Admin 대시보드

> 단일 self-contained HTML. CDN: Chart.js. Vanilla JS. CSS Grid + Dark theme.
> Bootstrap 사용하지 않음.

---

## 1. 엔드포인트

### D22-1: `GET /admin`
- 단일 HTML (인라인 CSS + JS)
- Chart.js CDN
- 5초 폴링

### D22-2: `GET /api/admin/status`
```json
{
  "status": "ok",
  "uptime_seconds": 3600,
  "timestamp": "2026-10-02T12:00:00Z",
  "engine": { "memory_limit": 64, "cpu_timeout_ms": 500, "memory_usage_mb": 1.2 },
  "database": { "backend": "postgres", "migrations_applied": [...] },
  "redis": { "enabled": true },
  "pipelines": { "board": "initialized", "erp": "initialized", ... },
  "socket": { "active_sessions": 3 },
  "auth": { "jwt_enabled": true, "rate_limit_per_min": 100 },
  "version": "1.0.0"
}
```

### D22-3: `GET /api/admin/metrics`
```json
{
  "http_requests": { "board": 10, "erp": 5, ... },
  "cache": { "hits": 100, "misses": 20, "hit_rate_percent": 83.3 },
  "hooks": { "executions": 50, "errors": 2 },
  "engine_memory_mb": 1.2,
  "active_sessions": 3
}
```

---

## 2. 대시보드 레이아웃 (Dark theme, CSS Grid)

```
┌─────────────────────────────────────────────────────────────┐
│  Header: "Meta Server Admin" + Status Dot + Uptime + Refresh │
├─────────────────────────────────────────────────────────────┤
│  KPI Cards Row:                                              │
│  [Status UP] [Uptime 1h] [DB: postgres] [Redis: ON] [Sessions: 3] │
├─────────────────────────────────────────────────────────────┤
│  Charts Row:                                                 │
│  [HTTP Requests by Domain — Bar Chart]                       │
│  [Cache Hit Rate — Doughnut]                                 │
│  [Hook Exec vs Errors — Line Chart]                          │
│  [Engine Memory — Gauge]                                     │
├─────────────────────────────────────────────────────────────┤
│  Status Table: Pipeline states + Auth + Rate Limit           │
├─────────────────────────────────────────────────────────────┤
│  Footer: version + last updated                              │
└─────────────────────────────────────────────────────────────┘
```

---

## 3. 파일 구조

| 파일 | 내용 |
|------|------|
| `src/admin.rs` (신규) | 대시보드 HTML + 상태/메트릭스 JSON 생성 |
| `src/http.rs` | `/admin`, `/api/admin/status`, `/api/admin/metrics` 추가 |
| `src/lib.rs` | `pub mod admin;` 추가 |
