# 개발 계획서 (Development Plan)

> Axum + Tokio + rquickjs 하이브리드 계층형 서버 구축 프로젝트
> 마지막 갱신: 2026-09-30

## 1. 목표 (Goal)

- **인프라 제어권**은 Rust가, **비즈니스 정책 제어권**은 JavaScript 스크립트가 가지도록 하는 하이브리드 서버를 구축한다.
- 컴파일 및 재배포 없이 DB/Redis 데이터 변경만으로 기능을 온/오프하는 구조를 구현한다.

## 2. 로드맵 (Roadmap)

### Phase 1: 인프라 뼈대 (Infrastructure Skeleton)
- **1.1** Axum 서버 스캐폴딩 및 Tokio 런타임 구성
- **1.2** rquickjs 런타임 초기화 (`src/engine`)
- **1.3** 샌드박스 리소스 제한 (CPU Timeout, Memory Limit - Throttling)

### Phase 2: 바인딩 레이어 (Binding Layer)
- **2.1** `rust_` 접두사 SDK 함수 구현 (`src/sdk`)
- **2.2** rquickjs 컨텍스트 주입 (Binding) 구조
- **2.3** 패닉 방지 및 `Result<T, rquickjs::Error>` 반환 강제 구조

### Phase 3: 파이프라인 체이닝 (Pipeline Chaining)
- **3.1** L1 인메모리 스크립트 캐시 적재기
- **3.2** 게시판 `before_create` 훅 발동 로직 (DB 저장 직전)
- **3.3** Axum 핸들러 파이프라인 연동 및 에러 코드(CJ1102 등) 반환

## 3. 현재 진행 상태 (Status)

| Phase | Task | 상태 | 비고 |
|-------|------|------|------|
| P1 | 1.1 서버 스캐폴딩 | ⏳ 미착수 | |
| P1 | 1.2 런타임 초기화 | ⏳ 미착수 | |
| P1 | 1.3 리소스 제한 | ⏳ 미착수 | |
| P2 | 2.1 SDK 함수 | ⏳ 미착수 | |
| P2 | 2.2 바인딩 구조 | ⏳ 미착수 | |
| P2 | 2.3 패닉 방지 | ⏳ 미착수 | |
| P3 | 3.1 L1 캐시 적재 | ⏳ 미착수 | |
| P3 | 3.2 훅 발동 로직 | ⏳ 미착수 | |
| P3 | 3.3 핸들러 파이프라인 | ⏳ 미착수 | |

**상태 도형:**
- ⏳ 미착수 (Not Started)
- 🔄 진행 중 (In Progress)
- ✅ 완료 (Completed)
- ⛔ 중단 (Blocked)

## 4. 상태 관리 규칙 (Status Management Rules)

1. 각 Phase의 모든 Task가 완료되면 `STATUS.md`의 상응하는 섹션에 완료 기준 및 체크리스트를 갱신한다.
2. Task 진행 중 단절이 발생하면 `STATUS.md`의 `[WIP 체크포인트]` 섹션에 중단 위치를 기록하고 상태 도형을 🔄 로 변경한다.
3. 다음 개발자가 `STATUS.md`의 `진행 지침`만 참고하여 단절된 Task를 이어서 수행할 수 있도록 유지한다.
