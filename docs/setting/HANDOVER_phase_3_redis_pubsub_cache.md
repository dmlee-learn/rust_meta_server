# Phase 3 이어서 개발하기 (Pipeline Chaining Handover)

> **대상:** Phase 3 (파이프라인 체이닝)를 **중단했다가 이어서 진행하는** 누구든.
> Phase 1(런타임+Throttling)과 Phase 2(SDK 바인딩)가 완료된 상태에서 이어서 진행한다.

---

## 1. Phase 3 목표 (Goal)

게시판 글 작성 요청이 들어오면, **DB에 저장하기 직전** L1 인메모리 캐시에 적재된 JS 스크립트의 `before_create` 훅을 찾아 실행하고, 그 결과에 따라 **통과(DB 저장)** 또는 **에러 코드 반환(CJ1102 등)**을 결정하는 Axum 핸들러 파이프라인을 완성하는 것이다.

**핵심 원칙 (AGENTS.md 준수):**
- **불변적 컨텍스트:** 요청 처리 중 스크립트 버전이 바뀌어도, 시작된 트랜잭션은 처음 바인딩된 스냅샷으로 끝까지 수행
- **캐시 스탬피드 방어:** L1 캐시 미스 시 Single Flight(뮤텍스 락)로 첫 1개 요청만 Redis에서 로드
- **결정적 실패 보호:** 구문 오류(CJ1101) 스크립트가 배포되면 즉시 직전 안전 버전으로 후퇴
- **에러 코드:** `CJ1101`(구문 오류), `CJ1102`(실행 시간 초과), `CJ1103`(메모리 초과)

## 2. 기대 폴더 구조 (Expected Layout)

```
src/
├── meta/
│   ├── mod.rs          # 메타데이터 모델 + L1 캐시
│   ├── cache.rs        # L1 인메모리 캐시 + Single Flight
│   └── script_loader.rs# Redis → L1 스크립트 로딩, 버전 바인딩
├── core/
│   └── handlers.rs     # Axum 핸들러 (게시판 글 생성)
scripts/
├── base/
│   └── board.js        # 기본 게시판 before_create 훅 (기준 파이프라인)
└── extensions/
    └── notice.js       # 공지사항 확장 (base 상속)
```

## 3. 핵심 구현 스펙 (Implementation Spec)

### 3.1 L1 캐시 + Single Flight (`src/meta/cache.rs`)

```rust
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::Mutex as TokioMutex;
use rquickjs::{Runtime, Context, Value};

/// 스크립트 버전 스냅샷 (불변적 컨텍스트 보장용)
#[derive(Clone)]
pub struct ScriptSnapshot {
    pub version: u64,          // 낙관적 버전 번호
    pub context: Context,      // 이미 초기화된 rquickjs Context
    pub globals: Value,
}

pub struct L1Cache {
    // key: 스크립트 모듈 ID (예: "base.board")
    // value: 현재 활성화된 스냅샷
    scripts: HashMap<String, Arc<ScriptSnapshot>>,
    // Single Flight: 동시에 같은 키를 로드하는 요청을 1개로 병합
    in_flight: HashMap<String, Arc<TokioMutex<()>>>,
}

impl L1Cache {
    /// 캐시 미스 시 Single Flight로 Redis에서 로드
    /// - 락이 이미 걸려 있으면 다른 요청은 그 결과를 대기 (Redis 접근 1회만)
    pub async fn get_or_load(&self, key: &str) -> Result<Arc<ScriptSnapshot>, ScriptError> {
        if let Some(s) = self.scripts.get(key) { return Ok(s.clone()); }
        // ... Single Flight 락 획득 후 Redis 로드, 구문 검증(CJ1101 차단)
    }
}
```

### 3.2 훅 실행 (`src/meta/script_loader.rs`)

```rust
/// 게시판 글 생성 전, before_create 훅 실행
/// - 통과: Ok(payload) → DB 저장 진행
/// - 실패: Err(code) → CJ1102 등 에러 코드 반환, DB 저장 차단
pub fn invoke_before_create(
    snapshot: &ScriptSnapshot,
    payload: serde_json::Value,
) -> Result<serde_json::Value, HookError> {
    snapshot.context.with(|ctx| {
        // 1. before_create 함수 존재 확인 (없으면 통과)
        // 2. fn(payload) 호출
        // 3. CPU Timeout(CJ1102) 또는 메모리 초과(CJ1103) 예처리기 감지
        // 4. JS throw된 에러 JSON {code, message} 파싱
        Ok(payload) // 통과 시 원본(또는 수정된) payload 반환
    })
}
```

### 3.3 Axum 핸들러 (`src/core/handlers.rs`)

```rust
#[post("/api/board/{board_id}")]
async fn create_board_post(
    State(state): State<AppState>,
    Path(board_id): Path<String>,
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    // 1. L1 캐시에서 스크립트 스냅샷 조회 (Single Flight)
    let snapshot = match state.l1_cache.get_or_load(&format!("board.{board_id}")).await {
        Ok(s) => s,
        Err(e) => return err_response(&e), // CJ1101 등
    };

    // 2. DB 저장 직전: before_create 훅 실행
    let result = invoke_before_create(&snapshot, payload);
    match result {
        Ok(modified_payload) => {
            // 3. 통과: DB 트랜잭션 저장 → 201 Created
            db_create(board_id, modified_payload).await
        }
        Err(e) => {
            // 4. 실패: CJ1102 등 에러 코드 JSON 반환 → 4xx
            (StatusCode::UNPROCESSABLE_ENTITY, Json(e.to_json())).into_response()
        }
    }
}
```

### 3.4 JS 스크립트 훅 예시 (`scripts/base/board.js`)

```javascript
// 기준 게시판 before_create 훅
function before_create(payload) {
    // 제목 검증 (명령형 로직 - 스크립트 영역)
    if (!payload.title || payload.title.length === 0) {
        throw new Error(JSON.stringify({ code: "CJ1104", message: "제목은 필수입니다." }));
    }

    // 제목 길이 제한 (상태/제약은 메타데이터, 알고리즘은 스크립트)
    const MAX_TITLE = 100;
    if (payload.title.length > MAX_TITLE) {
        throw new Error(JSON.stringify({ code: "CJ1104", message: "제목은 100자 이내여야 합니다." }));
    }

    // Rust SDK 활용: 게시일 기준 세금/할인 계산 등 필요 시
    // payload.price = rust_calculate_base_tax(payload.price);

    return payload; // 통과: 원본 또는 수정된 payload
}
```

## 4. 검증 시나리오 (Verification)

| # | 시나리오 | 요청 | 기대 결과 |
|---|----------|------|-----------|
| 1 | 정상 생성 | 유효한 title+content POST | `before_create` 통과 → DB 저장 → 201 |
| 2 | 비즈니스 룰 위반 | title 빈 값 POST | CJ1104 에러 JSON, 422, DB 미저장 |
| 3 | 무한 루프 훅 | `while(true){}` 포함 스크립트 배포 | CPU Timeout → CJ1102, 서버 생존 |
| 4 | 구문 오류 스크립트 | `function before_create(` 배포 | CJ1101 차단 → 직전 안전 버전 후퇴 |
| 5 | 버전 불변 | 요청 도중 스크립트 재배포 | 기존 요청은 원래 스냅샷으로 완료 |
| 6 | 캐시 스탬피드 | L1 miss + 동시 100개 요청 | Redis 로드 1회만 (Single Flight 확인) |

## 5. 단절 시 체크포인트 기록 양식

```
### 현재 상태
- 진행 중 Task: P3-3.2 (before_create 훅 발동 로직)
- 중단 위치: src/meta/script_loader.rs, invoke_before_create() 58번째 라인
- 다음 액션:
  1. JS throw된 에러 객체에서 {code, message} 파싱 로직 완성
  2. CPU Timeout 예외를 CJ102로 매핑하는 에러 처리 분기 추가
  3. tests/pipeline_before_create.rs 시나리오 1~3 작성
- 주의사항:
  - Context.with() 클로저는 단축 평가. 훅 실행 중 메모리 초과 시 예외가 panic으로 변환되지 않도록 확인
  - 낙관적 버전 바인딩: 스냅샷 version 필드를 DB 트랜잭션에도 함께 기록해 롤백 시 사용
```

## 6. 전체 Phase 완료 후 최종 검증 (Project Acceptance)

세 Phase 모두 완료 후, `docs/STATUS.md`의 `완료 기준` 전체 체크리스트를 확인하고 최종 연동 테스트를 수행한다:

1. [ ] `cargo run` 기동 → Phase 1 로그 확인
2. [ ] 테스트 스크립트 배포 → Phase 2 SDK 함수 JS에서 호출 확인
3. [ ] 게시판 글 생성 POST → Phase 3 훅 통과/차단 정상 동작 확인
4. [ ] `docs/STATUS.md`의 Phase별 완료 기준 모든 항목에 체크 표시
