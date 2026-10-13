// scripts/base/board.js — 기준 게시판 모듈
//
// AGENTS.md: scripts/base — 공통 뼈대 파이프라인.
// 확장 모듈(notice, qna, free)이 이 Base를 상속.
//
// 규약: `var api`를 선언하지 말고 인자 `api`에 직접 훅을 정의.
// (Base→Extension 바인딩 시 `api`가 함수 인자로 전달됨)

// before_create: 게시글 생성 전 검증 (제목 필수)
api.before_create = function (payload) {
    if (!payload.title) {
        throw new Error(JSON.stringify({ code: "SJ1201", message: "제목은 필수입니다." }));
    }
    return payload;
};

// after_save: 게시글 저장 후 후처리 (기본: 그대로 반환)
api.after_save = function (event) {
    return event;
};

// before_delete: 게시글 삭제 전 검증 (기본: 통과)
api.before_delete = function (post) {
    return true;
};
