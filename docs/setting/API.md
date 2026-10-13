# API Reference

> REST, WebSocket, and Metrics API for the MetaServer.

---

## 1. Authentication

All API endpoints (except `/health`, `/metrics`, and `/api/engine/*`) require
a valid JWT token in the `Authorization` header.

```
Authorization: Bearer <token>
```

Generate a token (HS256, secret from `JWT_SECRET` environment variable):

```bash
# Python
python3 -c "
import jwt, time
print(jwt.encode(
    {'sub': 'user1', 'domains': ['board', 'erp', 'blog', 'shop'], 'exp': int(time.time()) + 3600},
    'dev-secret-change-me',
    algorithm='HS256'
))"
```

**Claims:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `sub` | string | Yes | Subject (user identifier). |
| `domains` | string[] | No | Allowed domains. Omit for all access. |
| `exp` | int | Yes | Expiration (Unix timestamp, seconds). |

**Exempt paths:**

| Path | Reason |
|------|--------|
| `/health` | Infrastructure monitoring. |
| `/metrics` | Prometheus scraping. |
| `/api/engine/*` | Engine diagnostics. |

---

## 2. REST API

### 2.1 `POST /api/board`

Creates a board post. Validates the request through the `before_create` hook,
saves the post to the data store, and executes the `after_save` hook.

**Request**

```
POST /api/board HTTP/1.1
Authorization: Bearer <token>
Content-Type: application/json

{
  "title": "Notice title",
  "body": "Notice body text",
  "extra": { "custom": "value" }
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `title` | string | Yes | Post title. The `before_create` hook may reject empty titles (`SJ1201`). |
| `body` | string | Yes | Post body text. |
| `extra` | object | No | Arbitrary JSON payload passed to hooks. Defaults to `null`. |

**Response (Success: 201)**

```json
{
  "code": "OK",
  "id": 1,
  "title": "Notice title"
}
```

| Field | Type | Description |
|-------|------|-------------|
| `code` | string | `OK` on success. |
| `id` | number | Post ID assigned by the data store. |
| `title` | string | Post title (may be modified by hooks). |

**Response (Error)**

```json
{
  "code": "SJ1201",
  "message": "Title is required"
}
```

| HTTP Status | Code | Condition |
|-------------|------|-----------|
| 201 | `OK` | Success. |
| 400 | `CJ1104` | Business rule violation (JS `throw` with code). |
| 400 | `SJ1201` | Validation failure (e.g., empty title). |
| 401 | - | Missing or invalid JWT token. |
| 403 | - | Token does not include the required domain. |
| 404 | `CR0004` | Base or extension module not found in L3. |
| 429 | `CR4001` | Rate limit exceeded. |
| 500 | `CR0005` | Data store save failure. |
| 503 | `CJ1103` | Sandbox memory limit exceeded. |
| 504 | `CJ1102` | CPU timeout (infinite loop). |

---

### 2.2 `POST /api/erp/purchase-orders`

Creates an ERP purchase order.

**Request**

```
POST /api/erp/purchase-orders HTTP/1.1
Authorization: Bearer <token>
Content-Type: application/json

{
  "supplier": "Acme Corp",
  "amount": 150000,
  "currency": "KRW"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `supplier` | string | Yes | Supplier name. |
| `amount` | number | Yes | Order amount. |
| `currency` | string | No | Currency code. Defaults to `KRW`. |

**Response (Success: 201)**

```json
{
  "code": "OK",
  "domain": "erp",
  "result": { "id": 1 }
}
```

---

### 2.3 `POST /api/blog/articles`

Creates a blog article.

**Request**

```
POST /api/blog/articles HTTP/1.1
Authorization: Bearer <token>
Content-Type: application/json

{
  "title": "My Blog Post",
  "body": "Article content"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `title` | string | Yes | Article title. |
| `body` | string | Yes | Article content. |

**Response (Success: 201)**

```json
{
  "code": "OK",
  "domain": "blog",
  "result": { "id": 1 }
}
```

---

### 2.4 `POST /api/shop/orders`

Creates a shop order.

**Request**

```
POST /api/shop/orders HTTP/1.1
Authorization: Bearer <token>
Content-Type: application/json

{
  "product_id": 42,
  "quantity": 2,
  "price": 25000
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `product_id` | number | Yes | Product identifier. |
| `quantity` | number | Yes | Order quantity. |
| `price` | number | Yes | Unit price. |

**Response (Success: 201)**

```json
{
  "code": "OK",
  "domain": "shop",
  "result": { "id": 1 }
}
```

---

### 2.5 `GET /health`

Returns server health status. **No authentication required.**

**Response (200)**

```json
{
  "status": "ok",
  "engine": {
    "memory_limit": 268435456,
    "cpu_timeout_ms": 1500,
    "memory_usage": { "malloc_size": 1048576, "total_size": 2097152 }
  },
  "database": {
    "backend": "postgres",
    "migrations_applied": ["001_init.sql", "002_domain_tables.sql"]
  },
  "redis": { "enabled": true },
  "pipelines": {
    "board": true,
    "erp": true,
    "blog": true,
    "shop": true
  },
  "socket": { "active_sessions": 0 }
}
```

---

### 2.6 `GET /metrics`

Returns Prometheus text format metrics. **No authentication required.**

**Response (200, `text/plain; version=0.0.4`)**

```
# HELP meta_server_http_requests_total HTTP requests by endpoint
# TYPE meta_server_http_requests_total counter
meta_server_http_requests_total{endpoint="board"} 42
meta_server_http_requests_total{endpoint="erp"} 10
...

# HELP meta_server_cache_hits_total Cache hits by layer
# TYPE meta_server_cache_hits_total counter
meta_server_cache_hits_total{layer="l1"} 1000
...

# HELP meta_server_hook_executions_total Total hook executions
# TYPE meta_server_hook_executions_total counter
meta_server_hook_executions_total 500

# HELP meta_server_active_sessions Active WebSocket sessions
# TYPE meta_server_active_sessions gauge
meta_server_active_sessions 3

# HELP meta_server_engine_memory_bytes rquickjs engine memory usage
# TYPE meta_server_engine_memory_bytes gauge
meta_server_engine_memory_bytes 524288
```

---

## 3. WebSocket API

### 3.1 `GET /ws`

Upgrades the HTTP connection to WebSocket. The server executes JavaScript
socket hooks (`on_socket_connect`, `on_socket_message`, `on_socket_disconnect`)
for each event.

**Upgrade**

```
GET /ws HTTP/1.1
Upgrade: websocket
Connection: Upgrade
Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==
Sec-WebSocket-Version: 13
```

**Response (101 Switching Protocols)**

The server sends a `welcome` message immediately after the `on_socket_connect`
hook returns a value.

```json
{
  "type": "welcome",
  "data": "welcome"
}
```

### 3.2 Message Protocol

#### 3.2.1 Client → Server (Message)

```json
{
  "type": "message",
  "data": "hello from client"
}
```

#### 3.2.2 Server → Client (Reply)

```json
{
  "type": "reply",
  "data": "hello from server"
}
```

#### 3.2.3 Server → Client (Error)

```json
{
  "type": "error",
  "code": "CJ2101",
  "message": "Socket hook serialization failed"
}
```

#### 3.2.4 Server → Client (Close)

```json
{
  "type": "close",
  "code": "CR2001",
  "message": "Session pool limit exceeded"
}
```

### 3.3 Hook Lifecycle

| Event | Hook | Trigger |
|-------|------|---------|
| Connect | `on_socket_connect(payload)` | WebSocket connection established. |
| Message | `on_socket_message(payload)` | Client sends a message. |
| Disconnect | `on_socket_disconnect(payload)` | WebSocket connection closed. |

### 3.4 WebSocket Error Codes

| Code | Condition |
|------|-----------|
| `CR2001` | Session pool limit exceeded. |
| `CR2002` | WebSocket handshake failure. |
| `CJ2101` | Socket hook serialization or packet loss. |

---

## 4. Rate Limiting

The server enforces a per-IP request limit. When the limit is exceeded,
the server returns `429 Too Many Requests` with a `Retry-After` header.

**Configuration:**

| Environment Variable | Default | Description |
|---------------------|---------|-------------|
| `RATE_LIMIT_PER_MIN` | `60` | Maximum requests per minute per IP. |

**Response (429)**

```json
{
  "code": "CR4001",
  "message": "Rate limit exceeded",
  "retry_after_ms": 30000
}
```

Headers:

| Header | Description |
|--------|-------------|
| `Retry-After` | Seconds until the client may retry. |

**Exempt paths:** `/health`, `/metrics`, `/api/engine/*`

---

## 5. Error Code Reference

Format: `[Domain 1][Layer 1][Type 1][Number 3]` (6 characters).

| Position | Meaning | Values |
|----------|---------|--------|
| 1 | Domain | `C`=Core, `S`=Shop, `E`=ERP, `B`=Blog |
| 2 | Layer | `R`=Rust Core, `J`=JavaScript |
| 3 | Type | `0`=Infra, `1`=Runtime, `2`=Socket, `3`=Data, `4`=Security |
| 4-6 | Number | Sequential (001-999) |

### 5.1 Core (C)

| Code | Layer | Type | Description |
|------|-------|------|-------------|
| `CR0001` | R | 0 | System memory or thread pool allocation failure. |
| `CR0002` | R | 0 | rquickjs engine initialization failure. |
| `CR0004` | R | 0 | Module not found in L3. |
| `CR0005` | R | 0 | Data store save failure. |
| `CR2001` | R | 2 | Socket session pool limit exceeded. |
| `CR2002` | R | 2 | WebSocket handshake failure. |
| `CR4001` | R | 4 | Rate limit exceeded. |
| `CJ1101` | J | 1 | JS syntax error (detected at cache load). |
| `CJ1102` | J | 1 | CPU timeout (infinite loop). |
| `CJ1103` | J | 1 | Memory limit exceeded. |
| `CJ1104` | J | 1 | General JS exception (business rule). |
| `CJ2101` | J | 2 | Socket hook serialization or packet loss. |

### 5.2 Shop (S)

| Code | Layer | Type | Description |
|------|-------|------|-------------|
| `SJ1201` | J | 2 | Script calculation error (e.g., empty title). |
| `SJ1301` | J | 3 | Pinned post deletion attempt. |
| `SJ1401` | J | 4 | Insufficient delete permission. |

---

## 6. Examples

### 6.1 Create Board Post (with JWT)

```bash
TOKEN=$(python3 -c "
import jwt, time
print(jwt.encode(
    {'sub': 'user1', 'domains': ['board'], 'exp': int(time.time()) + 3600},
    'dev-secret-change-me',
    algorithm='HS256'
))")

curl -X POST http://localhost:3000/api/board \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"title":"Hello","body":"World","extra":{}}'
```

Response:

```json
{
  "code": "OK",
  "id": 1,
  "title": "Hello"
}
```

### 6.2 Health Check (no auth)

```bash
curl http://localhost:3000/health
```

### 6.3 Metrics (no auth)

```bash
curl http://localhost:3000/metrics
```

### 6.4 WebSocket Client (Node.js)

```javascript
const WebSocket = require("ws");

const ws = new WebSocket("ws://localhost:3000/ws");

ws.on("open", () => {
  console.log("Connected");
});

ws.on("message", (data) => {
  const msg = JSON.parse(data.toString());
  console.log("Received:", msg);

  if (msg.type === "welcome") {
    ws.send(JSON.stringify({ type: "message", data: "hello" }));
  }
});

ws.on("close", () => {
  console.log("Disconnected");
});
```
