/*
 * Phase 29: k6 부하 테스트 스크립트
 *
 * 실행: k6 run loadtest/k6.js
 * 환경변수: META_SERVER_URL (기본 http://localhost:3000), ADMIN_TOKEN
 *
 * 시나리오:
 *   - warmup: health check
 *   - ramp: 100 → 500 VUs (30s stage each)
 *   - steady: 500 VUs 30s
 *
 * metrics: p95/p99 latency, throughput, error rate
 */

import http from 'k6/http';
import { check, sleep } from 'k6';

const BASE = __ENV.META_SERVER_URL || 'http://localhost:3000';
const ADMIN_TOKEN = __ENV.ADMIN_TOKEN || '';

const headers = {
  'Content-Type': 'application/json',
};

function healthCheck() {
  const res = http.get(`${BASE}/health`, { headers });
  return check(res, {
    'health status 200': (r) => r.status === 200,
  });
}

function createBoard(i) {
  const res = http.post(
    `${BASE}/api/board`,
    JSON.stringify({
      title: `Load Test Post ${i}`,
      body: 'k6 load test body',
      extra: {},
    }),
    { headers }
  );
  return check(res, {
    'board 201': (r) => r.status === 201,
    'board code OK': (r) => {
      try {
        return r.json('code') === 'OK';
      } catch (e) {
        return false;
      }
    },
  });
}

function createErp(i) {
  const res = http.post(
    `${BASE}/api/erp/purchase-orders`,
    JSON.stringify({
      workflow_id: `wf-${i}`,
      supplier: 'LoadAcme',
      quantity: 5,
      amount: 500,
    }),
    { headers }
  );
  return check(res, {
    'erp 201': (r) => r.status === 201,
  });
}

function createBlog(i) {
  const res = http.post(
    `${BASE}/api/blog/articles`,
    JSON.stringify({
      title: `Load Article ${i}`,
      body: 'k6 load test',
      category: 'general',
    }),
    { headers }
  );
  return check(res, {
    'blog 201': (r) => r.status === 201,
  });
}

function createShop(i) {
  const res = http.post(
    `${BASE}/api/shop/orders`,
    JSON.stringify({
      product_id: `prod-${i % 100}`,
      quantity: 2,
      price: 1000,
    }),
    { headers }
  );
  return check(res, {
    'shop 201': (r) => r.status === 201,
  });
}

export const options = {
  scenarios: {
    warmup: {
      exec: ['healthCheck'],
      vus: 10,
      duration: '5s',
    },
    ramp: {
      exec: [
        'createBoard',
        'createErp',
        'createBlog',
        'createShop',
      ],
      startVUs: 100,
      target: 500,
      duration: '30s',
    },
    steady: {
      exec: [
        'createBoard',
        'createErp',
        'createBlog',
        'createShop',
      ],
      vus: 500,
      duration: '30s',
    },
  },
  thresholds: {
    http_req_duration: ['p(95)<500', 'p(99)<1000'],
    http_req_failed: ['rate<0.05'],
    checks: ['rate>0.95'],
  },
};

export default function () {
  const r = Math.floor(Math.random() * 1000000);
  switch (Math.floor(Math.random() * 4)) {
    case 0:
      createBoard(r);
      break;
    case 1:
      createErp(r);
      break;
    case 2:
      createBlog(r);
      break;
    default:
      createShop(r);
      break;
  }
  sleep(0.1);
}
