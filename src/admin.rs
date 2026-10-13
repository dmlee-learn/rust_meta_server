//! Phase 22: Admin 대시보드 + 서버 상태 API.
//!
//! - `GET /admin`: 단일 self-contained HTML (Chart.js CDN, dark theme, CSS Grid)
//! - `GET /api/admin/status`: 종합 서버 상태 JSON
//! - `GET /api/admin/metrics`: 메트릭스 JSON 스냅샷
//!
//! 에러 코드:
//! - CR5001: 대시보드 페이지 제공 실패 (CR5001)

use std::time::{SystemTime, UNIX_EPOCH};

/// 서버 기동 시각 (uptime 계산용).
pub fn uptime_seconds(started_at: &SystemTime) -> u64 {
    started_at
        .elapsed()
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 현재 시각을 ISO 8601 문자열로 반환.
pub fn now_iso() -> String {
    let now = SystemTime::now();
    let secs = now.duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    // Simple ISO-like: just use unix secs for now (sufficient for dashboard).
    format!("unix:{secs}")
}

/// 대시보드 HTML 페이지.
///
/// 단일 self-contained 파일: 인라인 CSS + JS + Chart.js CDN.
/// Phase 23: WebSocket 실시간 + 에러 로그 + 캐시 flush.
pub fn dashboard_html() -> &'static str {
    r#"<DOCTYPE html>
<html lang="ko">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Meta Server Admin</title>
<script src="https://cdn.jsdelivr.net/npm/chart.js@4.4.0/dist/chart.umd.min.js"></script>
<style>
:root {
  --bg: #0f1117;
  --surface: #1a1d27;
  --surface-2: #222633;
  --border: #2a2e3d;
  --text: #e2e4ed;
  --text-dim: #8b8fa8;
  --accent: #6c7cff;
  --accent-glow: rgba(108,124,255,0.25);
  --green: #34d399;
  --red: #f87171;
  --yellow: #fbbf24;
  --cyan: #22d3ee;
}
* { margin: 0; padding: 0; box-sizing: border-box; }
body {
  font-family: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
  background: var(--bg);
  color: var(--text);
  min-height: 100vh;
  padding: 1.5rem;
}
.container { max-width: 1400px; margin: 0 auto; }

/* Header */
.header {
  display: flex; align-items: center; justify-content: space-between;
  margin-bottom: 1.5rem; padding-bottom: 1rem;
  border-bottom: 1px solid var(--border);
}
.header h1 { font-size: 1.5rem; font-weight: 700; letter-spacing: -0.02em; }
.header .status-dot {
  display: inline-block; width: 10px; height: 10px; border-radius: 50%;
  margin-right: 8px; animation: pulse 2s infinite;
}
.header .status-dot.up { background: var(--green); box-shadow: 0 0 8px var(--green); }
.header .status-dot.down { background: var(--red); box-shadow: 0 0 8px var(--red); }
@keyframes pulse { 0%,100% { opacity: 1; } 50% { opacity: 0.5; } }
.header-right { display: flex; align-items: center; gap: 1rem; font-size: 0.85rem; color: var(--text-dim); }
.header-right .uptime { font-variant-numeric: tabular-nums; }
.refresh-btn {
  background: var(--surface-2); border: 1px solid var(--border); color: var(--text);
  padding: 0.4rem 0.8rem; border-radius: 6px; cursor: pointer; font-size: 0.8rem;
  transition: all 0.2s;
}
.refresh-btn:hover { background: var(--accent); color: white; border-color: var(--accent); }

/* KPI Cards */
.kpi-row {
  display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
  gap: 1rem; margin-bottom: 1.5rem;
}
.kpi-card {
  background: var(--surface); border: 1px solid var(--border);
  border-radius: 12px; padding: 1.2rem; position: relative; overflow: hidden;
}
.kpi-card::before {
  content: ''; position: absolute; top: 0; left: 0; right: 0; height: 2px;
  background: linear-gradient(90deg, var(--accent), transparent);
}
.kpi-card .label { font-size: 0.75rem; color: var(--text-dim); text-transform: uppercase; letter-spacing: 0.05em; margin-bottom: 0.5rem; }
.kpi-card .value { font-size: 1.8rem; font-weight: 700; font-variant-numeric: tabular-nums; }
.kpi-card .sub { font-size: 0.75rem; color: var(--text-dim); margin-top: 0.25rem; }
.kpi-card .badge {
  display: inline-block; padding: 2px 8px; border-radius: 4px;
  font-size: 0.7rem; font-weight: 600;
}
.badge-up { background: rgba(52,211,153,0.15); color: var(--green); }
.badge-down { background: rgba(248,113,113,0.15); color: var(--red); }
.badge-on { background: rgba(108,124,255,0.15); color: var(--accent); }
.badge-off { background: rgba(139,143,168,0.15); color: var(--text-dim); }

/* Charts Row */
.charts-row {
  display: grid; grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
  gap: 1rem; margin-bottom: 1.5rem;
}
.chart-card {
  background: var(--surface); border: 1px solid var(--border);
  border-radius: 12px; padding: 1.2rem;
}
.chart-card h3 { font-size: 0.9rem; font-weight: 600; color: var(--text); margin-bottom: 1rem; }
.chart-wrapper { position: relative; width: 100%; height: 220px; }
.chart-wrapper canvas { width: 100% !important; height: 100% !important; }

/* Status Table */
.status-section { margin-bottom: 1.5rem; }
.status-card {
  background: var(--surface); border: 1px solid var(--border);
  border-radius: 12px; padding: 1.2rem;
}
.status-card h3 { font-size: 0.9rem; font-weight: 600; margin-bottom: 1rem; }
.status-grid {
  display: grid; grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
  gap: 1rem;
}
.status-item { display: flex; justify-content: space-between; align-items: center; padding: 0.6rem 0; border-bottom: 1px solid var(--border); }
.heat-cell { border-radius: 8px; padding: 1rem; text-align: center; border: 1px solid var(--border); transition: background 0.3s; }
.heat-label { font-size: 0.85rem; color: var(--text-dim); margin-bottom: 0.5rem; }
.heat-value { font-size: 1.6rem; font-weight: 700; color: white; }
.status-item:last-child { border-bottom: none; }
.status-item .name { color: var(--text-dim); font-size: 0.85rem; }
.status-item .val { font-weight: 600; font-size: 0.85rem; font-variant-numeric: tabular-nums; }
.status-item .val.ok { color: var(--green); }
.status-item .val.warn { color: var(--yellow); }
.status-item .val.err { color: var(--red); }

/* Memory Bar */
.memory-bar-container { margin-top: 0.5rem; }
.memory-bar-bg { height: 8px; background: var(--surface-2); border-radius: 4px; overflow: hidden; }
.memory-bar-fill { height: 100%; border-radius: 4px; transition: width 0.5s ease; }

/* Footer */
.footer {
  margin-top: 2rem; padding-top: 1rem; border-top: 1px solid var(--border);
  display: flex; justify-content: space-between; font-size: 0.75rem; color: var(--text-dim);
}

/* Responsive */
@media (max-width: 768px) {
  .kpi-row { grid-template-columns: repeat(2, 1fr); }
  .charts-row { grid-template-columns: 1fr; }
}
</style>
</head>
<body>
<div class="container">
  <!-- Header -->
  <div class="header">
    <div style="display:flex;align-items:center;gap:0.75rem">
      <span class="status-dot up" id="statusDot"></span>
      <h1>Meta Server Admin</h1>
      <span class="badge badge-up" id="statusBadge">UP</span>
    </div>
    <div class="header-right">
      <span class="uptime">Uptime: <span id="uptime">--</span></span>
      <button class="refresh-btn" onclick="fetchAll()">⟳ Refresh</button>
    </div>
  </div>

  <!-- KPI Cards -->
  <div class="kpi-row">
    <div class="kpi-card">
      <div class="label">System Status</div>
      <div class="value" id="kpiStatus">--</div>
      <div class="sub">Server operational</div>
    </div>
    <div class="kpi-card">
      <div class="label">Database</div>
      <div class="value" id="kpiDb">--</div>
      <div class="sub" id="kpiDbSub">Backend</div>
    </div>
    <div class="kpi-card">
      <div class="label">Redis</div>
      <div class="value" id="kpiRedis">--</div>
      <div class="sub" id="kpiRedisSub">Cache layer</div>
    </div>
    <div class="kpi-card">
      <div class="label">Active Sessions</div>
      <div class="value" id="kpiSessions">--</div>
      <div class="sub">WebSocket connections</div>
    </div>
    <div class="kpi-card">
      <div class="label">Engine Memory</div>
      <div class="value" id="kpiMem">--</div>
      <div class="memory-bar-container">
        <div class="memory-bar-bg"><div class="memory-bar-fill" id="memBar" style="width:0%;background:var(--accent)"></div></div>
      </div>
      <div class="sub" id="kpiMemSub">Usage / Limit</div>
    </div>
  </div>

  <!-- Charts -->
  <div class="charts-row">
    <div class="chart-card">
      <h3>HTTP Requests by Domain</h3>
      <div class="chart-wrapper"><canvas id="httpChart"></canvas></div>
    </div>
    <div class="chart-card">
      <h3>Cache Hit Rate</h3>
      <div class="chart-wrapper"><canvas id="cacheChart"></canvas></div>
    </div>
    <div class="chart-card">
      <h3>Hook Executions vs Errors</h3>
      <div class="chart-wrapper"><canvas id="hookChart"></canvas></div>
    </div>
  </div>

  <!-- Status Table -->
  <div class="status-section">
    <div class="status-card">
      <h3>System Status Detail</h3>
      <div class="status-grid" id="statusGrid"></div>
    </div>
  </div>

  <!-- Phase 27: 도메인별 요청 히트맵 -->
  <div class="status-section">
    <div class="status-card">
      <h3>Domain Request Heatmap</h3>
      <div id="heatmap" style="display:grid;grid-template-columns:repeat(4,1fr);gap:10px">
        <div class="heat-cell" id="heat-board" style="background:rgba(76,201,240,0.15)">
          <div class="heat-label">Board</div>
          <div class="heat-value" id="heatBoardVal">0</div>
        </div>
        <div class="heat-cell" id="heat-erp" style="background:rgba(76,201,240,0.15)">
          <div class="heat-label">ERP</div>
          <div class="heat-value" id="heatErpVal">0</div>
        </div>
        <div class="heat-cell" id="heat-blog" style="background:rgba(76,201,240,0.15)">
          <div class="heat-label">Blog</div>
          <div class="heat-value" id="heatBlogVal">0</div>
        </div>
        <div class="heat-cell" id="heat-shop" style="background:rgba(76,201,240,0.15)">
          <div class="heat-label">Shop</div>
          <div class="heat-value" id="heatShopVal">0</div>
        </div>
      </div>
    </div>
  </div>

  <!-- Phase 27: 서버 제어 -->
  <div class="status-section">
    <div class="status-card">
      <h3>Server Controls</h3>
      <div style="display:flex;gap:10px;flex-wrap:wrap;margin-bottom:12px">
        <button class="refresh-btn" onclick="pingNow()">🔄 Ping Now</button>
        <button class="refresh-btn" onclick="exportJson()">📊 Export JSON</button>
        <button class="refresh-btn" id="alertBtn" onclick="toggleAlerts()">🔔 Enable Alerts</button>
      </div>
      <div id="controlResult" style="color:var(--text-dim);font-family:monospace;font-size:0.9em"></div>
    </div>
  </div>

  <!-- Phase 40: Extension (모듈) Management -->
  <div class="status-section">
    <div class="status-card">
      <h3 style="display:flex;justify-content:space-between;align-items:center">
        <span>Extension Modules <span class="badge badge-on" id="modCount">0</span></span>
        <button class="refresh-btn" onclick="loadModules()">📦 Load Modules</button>
      </h3>
      <div id="moduleList" style="max-height:320px;overflow-y:auto">
        <div style="color:var(--text-dim);text-align:center;padding:1.5rem">Click “Load Modules” to list</div>
      </div>
      <div style="margin-top:14px;padding-top:14px;border-top:1px solid var(--border)">
        <h4 style="margin:0 0 10px;font-size:0.95em">Deploy Script</h4>
        <div style="display:flex;gap:8px;flex-wrap:wrap;margin-bottom:8px">
          <input id="depModId" placeholder="module_id" style="flex:1;min-width:140px;background:var(--panel);border:1px solid var(--border);color:var(--text);padding:8px;border-radius:6px;font-family:monospace;font-size:0.85em"/>
          <button class="refresh-btn" onclick="deployModule()">🚀 Deploy</button>
        </div>
        <textarea id="depScript" rows="4" placeholder="api.before_create = function(p){ return p; };" style="width:100%;background:var(--panel);border:1px solid var(--border);color:var(--text);padding:8px;border-radius:6px;font-family:monospace;font-size:0.8em;resize:vertical"></textarea>
        <div id="deployResult" style="color:var(--text-dim);font-family:monospace;font-size:0.85em;margin-top:6px"></div>
      </div>
    </div>
  </div>

  <!-- Templates (Phase 41) -->
  <div class="status-section">
    <div class="status-card">
      <h3 style="display:flex;justify-content:space-between;align-items:center">
        <span>Templates (Hierarchical) <span class="badge badge-on" id="tplCount">0</span></span>
        <button class="refresh-btn" onclick="loadTemplates()">🧩 Load Templates</button>
      </h3>
      <div style="display:flex;gap:8px;flex-wrap:wrap;margin:10px 0">
        <input id="tplTenant" placeholder="tenant (default)" style="flex:1;min-width:140px;background:var(--panel);border:1px solid var(--border);color:var(--text);padding:8px;border-radius:6px;font-family:monospace;font-size:0.85em"/>
        <input id="tplId" placeholder="template_id (e.g. body.blog_post)" style="flex:2;min-width:200px;background:var(--panel);border:1px solid var(--border);color:var(--text);padding:8px;border-radius:6px;font-family:monospace;font-size:0.85em"/>
        <select id="tplDomain" style="background:var(--panel);border:1px solid var(--border);color:var(--text);padding:8px;border-radius:6px;font-size:0.85em">
          <option value="">all</option>
          <option value="board">board</option>
          <option value="blog">blog</option>
          <option value="shop">shop</option>
        </select>
        <button class="refresh-btn" onclick="loadTemplateTree()">🌳 Tree</button>
        <button class="refresh-btn" onclick="bumpTemplate()">⏫ Bump</button>
        <button class="refresh-btn" onclick="propagateTemplate()">🔁 Propagate</button>
      </div>
      <div id="templateList" style="max-height:320px;overflow-y:auto">
        <div style="color:var(--text-dim);text-align:center;padding:1.5rem">Click “Load Templates” to list</div>
      </div>
      <div id="templateTree" style="margin-top:14px;padding-top:14px;border-top:1px solid var(--border)"></div>
      <div style="margin-top:14px;padding-top:14px;border-top:1px solid var(--border)">
        <h4 style="margin:0 0 10px;font-size:0.95em">Instance Creation</h4>
        <div style="display:flex;gap:8px;flex-wrap:wrap;margin-bottom:8px">
          <select id="instTpl" style="flex:2;min-width:200px;background:var(--panel);border:1px solid var(--border);color:var(--text);padding:8px;border-radius:6px;font-family:monospace;font-size:0.85em">
            <option value="">— select template —</option>
          </select>
          <select id="instDomain" style="background:var(--panel);border:1px solid var(--border);color:var(--text);padding:8px;border-radius:6px;font-size:0.85em">
            <option value="board">board</option>
            <option value="blog">blog</option>
            <option value="shop">shop</option>
          </select>
          <select id="instPolicy" style="background:var(--panel);border:1px solid var(--border);color:var(--text);padding:8px;border-radius:6px;font-size:0.85em">
            <option value="frozen">frozen</option>
            <option value="live">live</option>
          </select>
        </div>
        <div style="display:flex;gap:8px;flex-wrap:wrap;margin-bottom:8px">
          <input id="instTitle" placeholder="title/name (optional, uses template default)" style="flex:3;min-width:240px;background:var(--panel);border:1px solid var(--border);color:var(--text);padding:8px;border-radius:6px;font-family:monospace;font-size:0.85em"/>
          <button class="refresh-btn" onclick="createInstance()">➕ Create Instance</button>
        </div>
        <div id="instResult" style="color:var(--text-dim);font-family:monospace;font-size:0.85em"></div>
      </div>
      <div style="margin-top:14px;padding-top:14px;border-top:1px solid var(--border)">
        <h4 style="margin:0 0 10px;font-size:0.95em">Site Provisioning</h4>
        <div style="display:flex;gap:8px;flex-wrap:wrap">
          <input id="provTenant" placeholder="target tenant_id" style="flex:1;min-width:140px;background:var(--panel);border:1px solid var(--border);color:var(--text);padding:8px;border-radius:6px;font-family:monospace;font-size:0.85em"/>
          <input id="provSiteTpl" placeholder="site template_id (e.g. site.shop)" style="flex:2;min-width:200px;background:var(--panel);border:1px solid var(--border);color:var(--text);padding:8px;border-radius:6px;font-family:monospace;font-size:0.85em"/>
          <button class="refresh-btn" onclick="applySiteTemplate()">🏗️ Apply</button>
        </div>
        <div id="provResult" style="color:var(--text-dim);font-family:monospace;font-size:0.85em;margin-top:6px"></div>
      </div>
    </div>
  </div>

  <!-- Error Logs -->
  <div class="status-section">
    <div class="status-card">
      <h3 style="display:flex;justify-content:space-between;align-items:center">
        <span>Recent Errors <span class="badge badge-off" id="logCount">0</span></span>
        <button class="refresh-btn" onclick="flushCache()">🗑️ Flush Cache</button>
      </h3>
      <div id="logList" style="max-height:300px;overflow-y:auto">
        <div style="color:var(--text-dim);text-align:center;padding:2rem">No errors recorded</div>
      </div>
    </div>
  </div>

  <!-- Footer -->
  <div class="footer">
    <span id="footerVersion">Meta Server v1.0.0</span>
    <span>Connection: <span id="wsStatus" style="color:var(--yellow)">POLLING</span></span>
    <span>Last updated: <span id="lastUpdated">--</span></span>
  </div>
</div>

<script>
let httpChart, cacheChart, hookChart;
let ws = null;
let pollingInterval = null;

function formatUptime(seconds) {
  if (seconds < 60) return seconds + 's';
  if (seconds < 3600) return Math.floor(seconds/60) + 'm ' + (seconds%60) + 's';
  if (seconds < 86400) return Math.floor(seconds/3600) + 'h ' + Math.floor((seconds%3600)/60) + 'm';
  return Math.floor(seconds/86400) + 'd ' + Math.floor((seconds%86400)/3600) + 'h';
}

function formatBytes(bytes) {
  if (bytes < 1024) return bytes + ' B';
  if (bytes < 1048576) return (bytes/1024).toFixed(1) + ' KB';
  if (bytes < 1073741824) return (bytes/1048576).toFixed(1) + ' MB';
  return (bytes/1073741824).toFixed(2) + ' GB';
}

function connectWebSocket() {
  const wsUrl = 'ws://' + window.location.host + '/ws/admin';
  ws = new WebSocket(wsUrl);

  ws.onopen = () => {
    document.getElementById('wsStatus').textContent = 'LIVE';
    document.getElementById('wsStatus').style.color = 'var(--green)';
    if (pollingInterval) { clearInterval(pollingInterval); pollingInterval = null; }
  };

  ws.onmessage = (event) => {
    try {
      const msg = JSON.parse(event.data);
      if (msg.type === 'metrics') {
        updateCharts(msg.data);
        document.getElementById('lastUpdated').textContent = new Date().toLocaleTimeString();
      }
    } catch(e) {}
  };

  ws.onclose = () => {
    document.getElementById('wsStatus').textContent = 'POLLING';
    document.getElementById('wsStatus').style.color = 'var(--yellow)';
    startPolling();
  };

  ws.onerror = () => {
    ws.close();
  };
}

function startPolling() {
  if (pollingInterval) return;
  fetchAll();
  pollingInterval = setInterval(fetchAll, 5000);
}

function fetchAll() {
  Promise.all([
    fetch('/api/admin/status', { headers: adminHeaders() }).then(r => r.json()),
    fetch('/api/admin/metrics', { headers: adminHeaders() }).then(r => r.json()),
    fetch('/api/admin/logs', { headers: adminHeaders() }).then(r => r.json())
  ]).then(([status, metrics, logs]) => {
    updateKPIs(status, metrics);
    updateCharts(metrics);
    updateStatusTable(status, metrics);
    updateLogs(logs);
    document.getElementById('lastUpdated').textContent = new Date().toLocaleTimeString();
  }).catch(e => {
    console.error('Fetch failed:', e);
    document.getElementById('statusDot').className = 'status-dot down';
    document.getElementById('statusBadge').textContent = 'DOWN';
    document.getElementById('statusBadge').className = 'badge badge-down';
  });
}

function flushCache() {
  fetch('/api/admin/cache/flush', { method: 'POST' })
    .then(r => r.json())
    .then(d => {
      alert(d.message || 'Cache flushed');
    })
    .catch(e => alert('Flush failed: ' + e));
}

// Phase 40: Extension (모듈) Management.
// Phase 42: 로그인 페이지에서 전달된 토큰을 sessionStorage에 저장.
(function() {
  const params = new URLSearchParams(window.location.search);
  const urlToken = params.get('token');
  if (urlToken) {
    sessionStorage.setItem('admin_token', urlToken);
    // URL에서 토큰 제거 (보안).
    history.replaceState({}, '', window.location.pathname);
  }
})();
function adminHeaders() {
  let t = sessionStorage.getItem('admin_token') || '';
  if (!t) t = (document.getElementById('adminToken') || {}).value || '';
  const h = { 'Content-Type': 'application/json' };
  if (t) h['x-admin-token'] = t;
  return h;
}

function loadModules() {
  fetch('/api/admin/modules', { headers: adminHeaders() })
    .then(r => r.json())
    .then(d => {
      if (d.code !== 'OK') { renderModules([]); return; }
      document.getElementById('modCount').textContent = d.count || 0;
      renderModules(d.modules || []);
    })
    .catch(e => {
      document.getElementById('moduleList').innerHTML =
        '<div style="color:var(--red);text-align:center;padding:1rem">Load failed: ' + e +
        '</div>';
    });
}

function renderModules(mods) {
  const el = document.getElementById('moduleList');
  if (!mods.length) {
    el.innerHTML = '<div style="color:var(--text-dim);text-align:center;padding:1.5rem">No modules</div>';
    return;
  }
  el.innerHTML = mods.map(m => {
    const cached = m.cached_in_l1 ? 'on' : 'off';
    const badgeCls = m.enabled ? 'badge-on' : 'badge-off';
    return '<div style="padding:0.6rem 0;border-bottom:1px solid var(--border)">'
      + '<div style="display:flex;justify-content:space-between;align-items:center;gap:8px">'
      + '<div style="min-width:0">'
      + '<span style="font-family:monospace;font-weight:600">' + m.module_id + '</span> '
      + '<span style="color:var(--text-dim);font-size:0.8em">v' + m.version + ' · ' + m.kind + '</span> '
      + '<span class="badge ' + badgeCls + '">' + (m.enabled ? 'enabled' : 'disabled') + '</span> '
      + '<span class="badge ' + (cached === 'on' ? 'badge-on' : 'badge-off') + '" style="opacity:0.7">L1:' + cached + '</span>'
      + '</div>'
      + '<div style="display:flex;gap:6px;flex-shrink:0">'
      + '<button class="refresh-btn" style="padding:4px 8px;font-size:0.75em" onclick="toggleModule(\'' + m.module_id + '\')">Toggle</button>'
      + '<button class="refresh-btn" style="padding:4px 8px;font-size:0.75em" onclick="reloadModule(\'' + m.module_id + '\')">Reload</button>'
      + '</div></div></div>';
  }).join('');
}

function toggleModule(id) {
  fetch('/api/admin/modules/' + encodeURIComponent(id) + '/toggle', { method: 'POST', headers: adminHeaders() })
    .then(r => r.json())
    .then(d => { alert(d.message || d.code); loadModules(); })
    .catch(e => alert('Toggle failed: ' + e));
}

function reloadModule(id) {
  fetch('/api/admin/modules/' + encodeURIComponent(id) + '/reload', { method: 'POST', headers: adminHeaders() })
    .then(r => r.json())
    .then(d => { alert(d.message || d.code); loadModules(); })
    .catch(e => alert('Reload failed: ' + e));
}

function deployModule() {
  const id = (document.getElementById('depModId') || {}).value || '';
  const src = (document.getElementById('depScript') || {}).value || '';
  if (!id || !src.trim()) {
    document.getElementById('deployResult').textContent = 'module_id and script_source required';
    return;
  }
  const res = document.getElementById('deployResult');
  res.textContent = 'Deploying ' + id + ' ...';
  fetch('/api/admin/modules/deploy', { method: 'POST', headers: adminHeaders(), body: JSON.stringify({ module_id: id, script_source: src }) })
    .then(r => r.json())
    .then(d => {
      res.textContent = (d.code === 'OK' ? '✅ ' : '❌ ') + (d.message || d.code);
      if (d.code === 'OK') loadModules();
    })
    .catch(e => { res.textContent = 'Deploy failed: ' + e; });
}

// ── Phase 41: Template management ──
function tplTenant() {
  return (document.getElementById('tplTenant') || {}).value || 'default';
}
function tplId() {
  return (document.getElementById('tplId') || {}).value || '';
}
function tplQuery() {
  return '?tenant=' + encodeURIComponent(tplTenant());
}

function loadTemplates() {
  fetch('/api/templates' + tplQuery(), { headers: adminHeaders() })
    .then(r => r.json())
    .then(d => {
      if (d.code !== 'OK') { renderTemplates([]); return; }
      document.getElementById('tplCount').textContent = d.count || 0;
      renderTemplates(d.templates || []);
    })
    .catch(e => {
      document.getElementById('templateList').innerHTML =
        '<div style="color:var(--red);text-align:center;padding:1rem">Load failed: ' + e + '</div>';
    });
}

function renderTemplates(tpls) {
  const el = document.getElementById('templateList');
  if (!tpls.length) {
    el.innerHTML = '<div style="color:var(--text-dim);text-align:center;padding:1.5rem">No templates</div>';
    // dropdown에도 초기화
    const dd = document.getElementById('instTpl');
    if (dd) { dd.innerHTML = '<option value="">— no templates —</option>'; }
    return;
  }
  el.innerHTML = tpls.map(t => {
    const badgeCls = t.enabled ? 'badge-on' : 'badge-off';
    return '<div style="padding:0.6rem 0;border-bottom:1px solid var(--border)">'
      + '<div style="display:flex;justify-content:space-between;align-items:center;gap:8px">'
      + '<div style="min-width:0">'
      + '<span style="font-family:monospace;font-weight:600">' + t.template_id + '</span> '
      + '<span style="color:var(--text-dim);font-size:0.8em">v' + t.version + ' · ' + t.ttype + ' · ' + t.domain + '</span> '
      + '<span class="badge ' + badgeCls + '">' + (t.enabled ? 'on' : 'off') + '</span> '
      + '<span class="badge badge-on" style="opacity:0.7">' + t.source + '</span>'
      + '</div></div></div>';
  }).join('');
  // Instance 생성 dropdown 갱신 (body 템플릿만)
  const dd = document.getElementById('instTpl');
  if (dd) {
    const bodies = tpls.filter(t => t.ttype === 'body');
    dd.innerHTML = '<option value="">— select body template —</option>'
      + bodies.map(t => '<option value="' + t.template_id + '">' + t.template_id + ' (v' + t.version + ')</option>').join('');
  }
}

function createInstance() {
  const tplId = (document.getElementById('instTpl') || {}).value || '';
  if (!tplId) { document.getElementById('instResult').textContent = 'Select a body template first.'; return; }
  const domain = (document.getElementById('instDomain') || {}).value || 'board';
  const policy = (document.getElementById('instPolicy') || {}).value || 'frozen';
  const title = (document.getElementById('instTitle') || {}).value || '';
  const tenant = tplTenant();
  const res = document.getElementById('instResult');
  res.textContent = 'Creating ' + domain + ' instance from ' + tplId + ' ...';

  const key = domain === 'shop' ? 'name' : 'title';
  const body = {};
  if (title) body[key] = title;

  const paths = { board: '/api/board/items', blog: '/api/blog/items', shop: '/api/shop/items' };
  const url = paths[domain] + '?template_id=' + encodeURIComponent(tplId) + '&tenant=' + encodeURIComponent(tenant) + '&sync_policy=' + policy;

  fetch(url, { method: 'POST', headers: { 'Content-Type': 'application/json', ...adminHeaders() }, body: JSON.stringify(body) })
    .then(r => r.json())
    .then(d => {
      if (d.code === 'OK') {
        res.textContent = '✅ Created id=' + d.id + ' (template: ' + d.template_id + ' v' + d.template_version + ', policy: ' + d.sync_policy + ')';
      } else {
        res.textContent = '❌ ' + (d.message || d.code);
      }
    })
    .catch(e => { res.textContent = '❌ Create failed: ' + e; });
}

function renderTree(node, depth) {
  depth = depth || 0;
  const pad = '&nbsp;'.repeat(depth * 4);
  let html = pad + '<span style="font-family:monospace;font-weight:' + (depth === 0 ? '700' : '400') + '">'
    + node.template.template_id + '</span> '
    + '<span style="color:var(--text-dim);font-size:0.8em">(' + node.template.ttype + ' · v' + node.template.version + ')</span>'
    + '<div>';
  (node.children || []).forEach(c => { html += renderTree(c, depth + 1); });
  html += '</div>';
  return html;
}

function loadTemplateTree() {
  const id = tplId();
  if (!id) { alert('template_id required'); return; }
  const el = document.getElementById('templateTree');
  el.innerHTML = '<div style="color:var(--text-dim)">Loading tree...</div>';
  fetch('/api/templates/' + encodeURIComponent(id) + '/tree' + tplQuery(), { headers: adminHeaders() })
    .then(r => r.json())
    .then(d => {
      if (d.code !== 'OK' || !d.template) { el.innerHTML = '<div style="color:var(--red)">' + (d.message || d.code) + '</div>'; return; }
      el.innerHTML = '<h4 style="margin:0 0 8px;font-size:0.95em">Tree</h4>' + renderTree(d.template, 0);
    })
    .catch(e => { el.innerHTML = '<div style="color:var(--red)">Tree failed: ' + e + '</div>'; });
}

function bumpTemplate() {
  const id = tplId();
  if (!id) { alert('template_id required'); return; }
  fetch('/api/admin/templates/' + encodeURIComponent(id) + '/bump' + tplQuery(), { method: 'POST', headers: adminHeaders() })
    .then(r => r.json())
    .then(d => { alert((d.code === 'OK' ? '✅ ' : '❌ ') + (d.message || d.code)); loadTemplates(); })
    .catch(e => alert('Bump failed: ' + e));
}

function propagateTemplate() {
  const id = tplId();
  if (!id) { alert('template_id required'); return; }
  const domain = (document.getElementById('tplDomain') || {}).value || '';
  const domainParam = domain ? '&domain=' + encodeURIComponent(domain) : '';
  fetch('/api/admin/templates/' + encodeURIComponent(id) + '/propagate' + tplQuery() + domainParam, { method: 'POST', headers: adminHeaders() })
    .then(r => r.json())
    .then(d => { alert((d.code === 'OK' ? '✅ ' : '❌ ') + (d.message || d.code) + ' (' + (d.propagated || 0) + ' instances, domain: ' + (d.domain || 'all') + ')'); })
    .catch(e => alert('Propagate failed: ' + e));
}

function applySiteTemplate() {
  const tenant = (document.getElementById('provTenant') || {}).value || '';
  const siteTpl = (document.getElementById('provSiteTpl') || {}).value || '';
  if (!tenant || !siteTpl) { document.getElementById('provResult').textContent = 'tenant + site template required'; return; }
  const res = document.getElementById('provResult');
  res.textContent = 'Applying ' + siteTpl + ' to ' + tenant + ' ...';
  fetch('/api/admin/tenants/' + encodeURIComponent(tenant) + '/apply-site-template?site_template_id=' + encodeURIComponent(siteTpl), { method: 'POST', headers: adminHeaders() })
    .then(r => r.json())
    .then(d => { res.textContent = (d.code === 'OK' ? '✅ ' : '❌ ') + (d.message || d.code) + ' (copied: ' + (d.copied || 0) + ')'; })
    .catch(e => { res.textContent = 'Provisioning failed: ' + e; });
}

// 토큰 입력 필드를 헤더에 붙여 다시 렌더링 (옵션).
function renderTokenField() {
  const section = document.querySelector('#moduleList');
  if (section && !document.getElementById('adminToken')) {
    const wrap = document.createElement('div');
    wrap.style.cssText = 'margin-bottom:10px';
    wrap.innerHTML = '<input id="adminToken" placeholder="x-admin-token (optional)" style="width:100%;max-width:280px;background:var(--panel);border:1px solid var(--border);color:var(--text);padding:6px;border-radius:6px;font-family:monospace;font-size:0.8em"/>';
    section.parentElement.insertBefore(wrap, section);
  }
}

function updateLogs(logsData) {
  const container = document.getElementById('logList');
  const count = document.getElementById('logCount');
  count.textContent = logsData.total || 0;
  if (!logsData.logs || logsData.logs.length === 0) {
    container.innerHTML = '<div style="color:var(--text-dim);text-align:center;padding:2rem">No errors recorded</div>';
    return;
  }
  container.innerHTML = logsData.logs.map(l => {
    const ts = l.timestamp ? new Date(l.timestamp * 1000).toLocaleTimeString() : '--';
    return '<div style="display:flex;justify-content:space-between;padding:0.5rem 0;border-bottom:1px solid var(--border);font-size:0.8rem">' +
      '<span><span style="color:var(--red);font-weight:600">' + l.code + '</span> ' +
      '<span style="color:var(--text-dim)">' + (l.domain || 'system') + '</span> ' +
      l.message + '</span>' +
      '<span style="color:var(--text-dim)">' + ts + '</span></div>';
  }).join('');
}

function updateKPIs(status, metrics) {
  // Status
  const isUp = status.status === 'ok';
  document.getElementById('statusDot').className = 'status-dot ' + (isUp ? 'up' : 'down');
  document.getElementById('statusBadge').textContent = isUp ? 'UP' : 'DOWN';
  document.getElementById('statusBadge').className = 'badge ' + (isUp ? 'badge-up' : 'badge-down');
  document.getElementById('kpiStatus').textContent = isUp ? 'UP' : 'DOWN';

  // Uptime
  document.getElementById('uptime').textContent = formatUptime(status.uptime_seconds);

  // Database
  document.getElementById('kpiDb').textContent = status.database.backend;
  document.getElementById('kpiDbSub').textContent = status.database.migrations_applied.length + ' migrations';

  // Redis
  document.getElementById('kpiRedis').textContent = status.redis.enabled ? 'ON' : 'OFF';
  document.getElementById('kpiRedisSub').textContent = status.redis.enabled ? 'Active' : 'Disabled';

  // Sessions
  document.getElementById('kpiSessions').textContent = status.socket.active_sessions;

  // Memory
  const memMb = (status.engine.memory_usage.mb || 0);
  const limitMb = (status.engine.memory_limit_mb || 64);
  const pct = Math.min(100, (memMb / limitMb) * 100);
  document.getElementById('kpiMem').textContent = memMb.toFixed(1) + ' MB';
  document.getElementById('kpiMemSub').textContent = memMb.toFixed(1) + ' / ' + limitMb + ' MB (' + pct.toFixed(1) + '%)';
  const bar = document.getElementById('memBar');
  bar.style.width = pct + '%';
  bar.style.background = pct > 80 ? 'var(--red)' : pct > 50 ? 'var(--yellow)' : 'var(--accent)';
}

function updateCharts(metrics) {
  // HTTP Requests
  const domains = ['board', 'erp', 'blog', 'shop'];
  const values = domains.map(d => (metrics.http_requests && metrics.http_requests[d]) || 0);
  if (!httpChart) {
    httpChart = new Chart(document.getElementById('httpChart'), {
      type: 'bar',
      data: {
        labels: domains,
        datasets: [{ label: 'Requests', data: values, backgroundColor: ['#6c7cff','#22d3ee','#34d399','#fbbf24'], borderRadius: 4 }]
      },
      options: {
        responsive: true, maintainAspectRatio: false,
        plugins: { legend: { display: false } },
        scales: {
          x: { grid: { display: false }, ticks: { color: '#8b8fa8' } },
          y: { beginAtZero: true, grid: { color: 'rgba(42,46,61,0.5)' }, ticks: { color: '#8b8fa8', precision: 0 } }
        }
      }
    });
  } else {
    httpChart.data.datasets[0].data = values;
    httpChart.update('none');
  }

  // Cache Hit Rate
  const hits = metrics.cache ? metrics.cache.hits : 0;
  const misses = metrics.cache ? metrics.cache.misses : 0;
  if (!cacheChart) {
    cacheChart = new Chart(document.getElementById('cacheChart'), {
      type: 'doughnut',
      data: {
        labels: ['Hits', 'Misses'],
        datasets: [{ data: [hits, misses], backgroundColor: ['#34d399', '#2a2e3d'], borderWidth: 0 }]
      },
      options: {
        responsive: true, maintainAspectRatio: false, cutout: '70%',
        plugins: {
          legend: { position: 'bottom', labels: { color: '#8b8fa8', padding: 15 } },
          tooltip: { callbacks: { label: ctx => ctx.label + ': ' + ctx.parsed + ' (' + (ctx.parsed / Math.max(1, hits+misses) * 100).toFixed(1) + '%)' } }
        }
      }
    });
  } else {
    cacheChart.data.datasets[0].data = [hits, misses];
    cacheChart.update('none');
  }

  // Hook Exec vs Errors
  const exec = metrics.hooks ? metrics.hooks.executions : 0;
  const errs = metrics.hooks ? metrics.hooks.errors : 0;
  if (!hookChart) {
    hookChart = new Chart(document.getElementById('hookChart'), {
      type: 'bar',
      data: {
        labels: ['Executions', 'Errors'],
        datasets: [{ label: 'Count', data: [exec, errs], backgroundColor: ['#22d3ee', '#f87171'], borderRadius: 4 }]
      },
      options: {
        responsive: true, maintainAspectRatio: false,
        plugins: { legend: { display: false } },
        scales: {
          x: { grid: { display: false }, ticks: { color: '#8b8fa8' } },
          y: { beginAtZero: true, grid: { color: 'rgba(42,46,61,0.5)' }, ticks: { color: '#8b8fa8', precision: 0 } }
        }
      }
    });
  } else {
    hookChart.data.datasets[0].data = [exec, errs];
    hookChart.update('none');
  }

  // Phase 27: 히트맵 업데이트
  updateHeatmap(metrics);
  // Phase 27: 에러 알림 체크
  checkAlerts(errs);
}

function updateHeatmap(metrics) {
  const reqs = (metrics && metrics.http_requests) || {};
  const vals = {
    board: reqs.board || 0,
    erp: reqs.erp || 0,
    blog: reqs.blog || 0,
    shop: reqs.shop || 0
  };
  const max = Math.max(1, vals.board, vals.erp, vals.blog, vals.shop);
  const cells = [
    { id: 'board', el: 'heat-board', valEl: 'heatBoardVal' },
    { id: 'erp', el: 'heat-erp', valEl: 'heatErpVal' },
    { id: 'blog', el: 'heat-blog', valEl: 'heatBlogVal' },
    { id: 'shop', el: 'heat-shop', valEl: 'heatShopVal' }
  ];
  cells.forEach(c => {
    const v = vals[c.id];
    const intensity = v / max;
    const alpha = 0.15 + intensity * 0.7;
    const el = document.getElementById(c.el);
    const valEl = document.getElementById(c.valEl);
    if (el) el.style.background = 'rgba(76,201,240,' + alpha.toFixed(2) + ')';
    if (valEl) valEl.textContent = v;
  });
}

// Phase 27: 서버 제어 함수
let alertsEnabled = false;
let lastHookErrors = 0;

async function pingNow() {
  const resultEl = document.getElementById('controlResult');
  resultEl.textContent = 'Pinging...';
  try {
    const adminToken = document.cookie ? '' : '';
    const res = await fetch('/api/admin/ping/trigger', { method: 'POST', headers: adminHeaders() });
    const data = await res.json();
    const dbInfo = data.db ? (data.db.backend + ' ' + data.db.latency_ms + 'ms') : '--';
    resultEl.textContent = 'DB: ' + dbInfo + ' | Redis: ' + (data.redis.enabled ? 'enabled' : 'disabled');
  } catch (e) {
    resultEl.textContent = 'Ping error: ' + e;
  }
}

async function exportJson() {
  try {
    const res = await fetch('/api/admin/export', { headers: adminHeaders() });
    const blob = await res.blob();
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = 'meta_server_metrics.json';
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
    const resultEl = document.getElementById('controlResult');
    resultEl.textContent = 'Exported JSON downloaded.';
  } catch (e) {
    const resultEl = document.getElementById('controlResult');
    resultEl.textContent = 'Export error: ' + e;
  }
}

async function toggleAlerts() {
  const btn = document.getElementById('alertBtn');
  if (!('Notification' in window)) {
    alert('Notification API not supported');
    return;
  }
  if (alertsEnabled) {
    alertsEnabled = false;
    btn.textContent = '🔔 Enable Alerts';
    return;
  }
  const perm = await Notification.requestPermission();
  if (perm === 'granted') {
    alertsEnabled = true;
    lastHookErrors = 0;
    btn.textContent = '🔕 Disable Alerts';
    new Notification('Meta Server', { body: 'Real-time alerts enabled' });
  } else {
    alert('Notification permission denied');
  }
}

function checkAlerts(currentErrors) {
  if (!alertsEnabled) return;
  if (currentErrors > lastHookErrors && lastHookErrors > 0) {
    new Notification('Meta Server Alert', {
      body: 'New hook errors detected: ' + (currentErrors - lastHookErrors) + ' new'
    });
  }
  if (currentErrors > lastHookErrors) {
    lastHookErrors = currentErrors;
  }
}

function updateStatusTable(status, metrics) {
  const items = [
    { name: 'Engine Memory Limit', val: status.engine.memory_limit_mb + ' MB', cls: 'ok' },
    { name: 'Engine CPU Timeout', val: status.engine.cpu_timeout_ms + ' ms', cls: 'ok' },
    { name: 'Engine Memory Usage', val: (status.engine.memory_usage.mb || 0).toFixed(1) + ' MB', cls: (status.engine.memory_usage.mb > status.engine.memory_limit_mb * 0.8) ? 'warn' : 'ok' },
    { name: 'Database Backend', val: status.database.backend, cls: 'ok' },
    { name: 'Migrations Applied', val: status.database.migrations_applied.length + ' files', cls: 'ok' },
    { name: 'Redis Enabled', val: status.redis.enabled ? 'Yes' : 'No', cls: status.redis.enabled ? 'ok' : 'warn' },
    { name: 'Board Pipeline', val: status.pipelines.board, cls: status.pipelines.board === 'initialized' ? 'ok' : 'err' },
    { name: 'ERP Pipeline', val: status.pipelines.erp, cls: status.pipelines.erp === 'initialized' ? 'ok' : 'err' },
    { name: 'Blog Pipeline', val: status.pipelines.blog, cls: status.pipelines.blog === 'initialized' ? 'ok' : 'err' },
    { name: 'Shop Pipeline', val: status.pipelines.shop, cls: status.pipelines.shop === 'initialized' ? 'ok' : 'err' },
    { name: 'JWT Auth', val: status.auth.jwt_enabled ? 'Enabled' : 'Disabled', cls: status.auth.jwt_enabled ? 'ok' : 'warn' },
    { name: 'Rate Limit', val: status.auth.rate_limit_per_min + ' req/min', cls: 'ok' },
    { name: 'Cache Hit Rate', val: (metrics.cache ? metrics.cache.hit_rate_percent : 0).toFixed(1) + '%', cls: (metrics.cache && metrics.cache.hit_rate_percent > 80) ? 'ok' : 'warn' },
    { name: 'Hook Errors', val: (metrics.hooks ? metrics.hooks.errors : 0) + ' total', cls: (metrics.hooks && metrics.hooks.errors === 0) ? 'ok' : 'warn' },
    { name: 'Active Sessions', val: status.socket.active_sessions + ' connections', cls: 'ok' },
    { name: 'Server Version', val: status.version, cls: 'ok' },
  ];
  document.getElementById('statusGrid').innerHTML = items.map(i =>
    '<div class="status-item"><span class="name">' + i.name + '</span><span class="val ' + i.cls + '">' + i.val + '</span></div>'
  ).join('');
}

// Phase 23: WebSocket 우선, 실패 시 polling 폴백
connectWebSocket();
setTimeout(() => { if (!ws || ws.readyState !== WebSocket.OPEN) startPolling(); }, 3000);
// Phase 40: 모듈 관리 토큰 필드 렌더
renderTokenField();
// 첫 로드
fetchAll();
</script>
</body>
</html>"#
}

/// 대시보드 HTML을 axum Response로 반환.
pub fn dashboard_response() -> axum::response::Response {
    use axum::response::{IntoResponse, Response};
    Response::builder()
        .header(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(axum::body::Body::from(dashboard_html().to_string()))
        .unwrap()
}
