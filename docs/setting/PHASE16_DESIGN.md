# Phase 16 설계: Docker 실 검증 + 실 PostgreSQL/Redis 통합

> Docker ERP/Blog/Shop crash 원인 진단, 실 PostgreSQL/Redis Docker 통합 테스트.

---

## 1. crash 원인 진단

### 1.1 증상
- Docker 컨테이너에서 `CJ1104`(JS throw) 에러가 발생하면 컨테이너가 crash (exit code 255).
- 정상 응답(`{"code":"OK",...}`)은 문제없이 반환.

### 1.2 원인
- rquickjs C 라이브러리가 JS 예외(throw) 처리 시 시그널을 올바르게 처리하지 못함.
- Docker 컨테이너 환경에서 C 라이브러리의 시그널 처리와 충돌.
- **Board pipeline도 동일한 구조이므로, Board의 `SJ1201` throw 시에도 동일한 crash 발생 가능.**

### 1.3 해결 방안

#### O16-1: Docker `--restart=always` (권장)
- crash 후 자동 재기동.
- 운영 환경에서 즉시 적용 가능.
- 근본 해결은 아니지만 서비스 가용성 확보.

#### O16-2: rquickjs 샌드박스 별도 쓰레드
- JS eval을 별도 Rust 쓰레드에서 실행.
- crash가 컨테이너 전체를 종료시키지 않도록 분리.
- 구현 복잡도 높음.

#### O16-3: rquickjs 버그 보고 + 업그레이드
- rquickjs 0.14 C 라이브러리 시그널 처리 버그 보고.
- 새 버전에서 수정 확인.
- 단기 해결 불가.

**결정: O16-1 즉시 적용 + O16-3 장기 추적**

---

## 2. 실 PostgreSQL/Redis Docker 통합 테스트

### 2.1 구성
- Docker Compose: PostgreSQL + Redis
- meta_server 컨테이너: `--network meta_server_meta-net`
- `DATABASE_URL=postgres://meta:metapass@postgres:5432/meta`
- `REDIS_URL=redis://:metapass@redis:6379`
- `REDIS_ENABLED=true`

### 2.2 테스트 항목
1. 서버 기동 → PostgreSQL 연결 + 마이그레이션 자동 적용
2. Redis 연결 확인
3. `POST /api/board` → PostgreSQL에 저장 확인
4. `GET /health` → `database.backend=postgres`, `redis.enabled=true`
5. `GET /metrics` → Prometheus text
6. `POST /api/erp/purchase-orders` → PostgreSQL에 저장
7. PostgreSQL에서 데이터 확인 (`SELECT * FROM board_posts`)

---

## 3. 산출물

| 파일 | 내용 |
|------|------|
| `docker-compose.yml` | meta_server 서비스 추가 |
| `tests/phase16_real_integration.rs` | 실 PostgreSQL/Redis Docker 통합 테스트 |
| `docs/setting/PHASE16_DESIGN.md` | 본 문서 |
