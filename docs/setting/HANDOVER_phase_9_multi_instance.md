# HANDOVER: Phase 9 — 멀티인스턴스 L1 무효화 시뮬레이션

> 이 문서는 Phase 9를 완료하고 단절했을 때, 다음 개발자(또는 다음 세션의 AI 에이전트)가
> 이어서 작업을 진행할 수 있도록 작성된 핸드오버 문서입니다.

---

## 1. Phase 9 완료 내용

### 1.1 멀티인스턴스 L1 무효화 시뮬레이션

**목표:** Docker가 없는 환경에서 `InMemoryPubSub`을 공유하는 2개 `BoardPipeline` 인스턴스로
L1 캐시 무효화 동기를 실증.

**구조:**
```
                    ┌─────────────────────┐
                    │  MemoryStore (L3)   │  ← 공유
                    └────────┬────────────┘
                             │
                    ┌────────┴────────────┐
                    │ InMemoryPubSub (L2) │  ← 공유
                    └───┬────────────┬────┘
                        │            │
              ┌─────────┴──┐  ┌──────┴─────────┐
              │ Pipeline A  │  │  Pipeline B     │
              │ L1Cache A   │  │  L1Cache B      │
              └────────────┘  └─────────────────┘
```

**시나리오:**
1. 인스턴스 A + B가 같은 `InMemoryPubSub` + `MemoryStore` 공유
2. A에서 POST → L1 A 적재, B에서 POST → L1 B 적재
3. A에서 `invalidate("base.board")` → L1 A + L2(공유)에서 삭제
4. L3에 v2 upsert → A/B 모두 `get_module` 시 v2 재적재
5. 동시 50개 요청 → Single Flight 동작 확인

### 1.2 테스트 파일

| 파일 | 테스트 수 | 내용 |
|------|----------|------|
| `tests/phase9_multi_instance.rs` | 6개 | 멀티인스턴스 L1 무효화 시뮬레이션 |

**테스트 항목:**
| 테스트 | 내용 |
|--------|------|
| `instance_a_creates_and_l1_populated` | 인스턴스 A 생성 → L1 적재 |
| `both_instances_l1_populated_then_invalidated` | 두 인스턴스 L1 적재 → 무효화 |
| `instance_a_invalidation_deletes_l2` | A 무효화 시 공유 L2에도 삭제 |
| `l3_new_version_both_instances_reload` | L3 v2 upsert → 두 인스턴스 v2 재적재 |
| `pubsub_publish_invalidation_received` | Pub/Sub 발행 → L1 무효화 |
| `single_flight_two_instances` | 동시 50개 요청 → Single Flight |

### 1.3 API 문서 수정

- `POST /api/board` 성공 응답 코드를 `200` → `201 Created`로 수정
- `docs/setting/API.md`에 반영

### 1.4 전체 테스트: **61개 통과**

| 테스트 스위트 | 개수 |
|---|---|
| lib | 11 |
| functional_http | 5 |
| integration_engine | 7 |
| phase2_loader | 10 |
| phase3_pipeline | 6 |
| phase5_socket | 3 |
| phase6_cache_flow | 2 |
| phase6_benchmark | 5 |
| phase7_benchmark | 6 |
| phase9_multi_instance | 6 |
| **합계** | **61** |

(Phase 8 실 연동 테스트 6개는 `#[ignore]` — Docker 필요)

---

## 2. 주의사항

### 2.1 `handle_create` 성공 코드: 201 Created

`BoardPipeline::handle_create`은 성공 시 `StatusCode::CREATED` (201)를 반환합니다.
테스트에서 `200`이 아닌 `201`을 확인해야 합니다.

### 2.2 InMemoryPubSub broadcast 한계

`InMemoryPubSub`은 `tokio::sync::broadcast` 채널을 사용.
`publish_invalidation` 호출 시 **동일 프로세스 내**의 수신자만 수신합니다.
실 Redis Pub/Sub은 **프로세스 간** 무효화를 지원합니다.
멀티인스턴스 테스트는 InMemoryPubSub의 in-process 한계 내에서 시뮬레이션합니다.

### 2.3 공유 L1Cache

두 `BoardPipeline` 인스턴스는 **별도** `L1Cache`를 가집니다.
공유하는 것은 `InMemoryPubSub` (L2)과 `MemoryStore` (L3)입니다.
실 환경에서는 각 인스턴스가 자체 L1을 갖고, Redis Pub/Sub으로 L1 무효화를 동기화합니다.

---

## 3. 다음 단계 (Phase 10)

### 3.1 실 Redis/PostgreSQL 연동 (Docker)
- Docker Desktop 설치 후 `docker compose up -d`
- `cargo test --test phase8_real_integration -- --ignored`
- 실 환경에서 L1→L2(Redis)→L3(PG) 3단계 캐시 실증

### 3.2 추가 도메인 모듈
- `scripts/base/` + `scripts/extensions/`에 ERP, Blog, Shop 모듈
- AGENTS.md의 도메인 확장 스크립트

### 3.3 SDK 확장
- `src/sdk`에 추가 Rust 함수
- `rust_calculate_base_tax()`, `rust_send_notification()` 등

### 3.4 운영 배포
- `cargo build --release`
- Docker 이미지 빌드
- 실 환경 배포

---

## 4. 빌드 및 테스트

```bash
# 전체 테스트 (61개)
cargo test

# Phase 9 멀티인스턴스 테스트
cargo test --test phase9_multi_instance -- --nocapture

# Phase 8 실 연동 테스트 (Docker 필요)
docker compose up -d
cargo test --test phase8_real_integration -- --ignored --nocapture
docker compose down
```
