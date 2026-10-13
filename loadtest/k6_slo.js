/*
 * Phase 40 SLO 벤치마크 (안정화 부하)
 * 50 VU, 30초. 기존 loadtest/k6.js의 500 VU를 이 불안정한 Docker 환경에서
 * 재시작 리스크를 줄이기 위해 낮춘 버전.
 *
 * 실행: META_SERVER_URL=http://meta_server:3000 (동일 compose 네트워크)
 */
import http from 'k6/http';
import { check, sleep } from 'k6';

const BASE = __ENV.META_SERVER_URL || 'http://meta_server:3000';
const headers = { 'Content-Type': 'application/json' };
const adminHeaders = {
  'Content-Type': 'application/json',
  'x-admin-token': __ENV.ADMIN_TOKEN || '',
};

function healthCheck() {
  const res = http.get(`${BASE}/health`, { headers });
  return check(res, { 'health 200': (r) => r.status === 200 });
}

function listModules() {
  const res = http.get(`${BASE}/api/admin/modules`, { headers: adminHeaders });
  return check(res, { 'modules ok': (r) => r.status === 200 });
}

function createBoard(i) {
  const res = http.post(
    `${BASE}/api/board`,
    JSON.stringify({ title: `SLO ${i}`, body: 'slo bench', extra: {} }),
    { headers },
  );
  return check(res, { 'board ok': (r) => r.status === 201 });
}

function listPosts() {
  const res = http.get(`${BASE}/api/tenant/posts`, { headers });
  return check(res, { 'posts ok': (r) => r.status === 200 });
}

export const options = {
  vus: 50,
  duration: '30s',
  thresholds: {
    http_req_duration: ['p(95)<500', 'p(99)<1000'],
    http_req_failed: ['rate<0.05'],
  },
};

export default function () {
  const r = Math.floor(Math.random() * 100000);
  switch (Math.floor(Math.random() * 4)) {
    case 0: createBoard(r); break;
    case 1: listPosts(); break;
    case 2: listModules(); break;
    default: healthCheck(); break;
  }
  sleep(0.05);
}
