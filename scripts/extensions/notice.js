// scripts/extensions/notice.js — 공지사항 확장 모듈
//
// AGENTS.md: scripts/extensions — Base를 상속·조립하는 도메인 확장.
// 기준: scripts/base/board.js
//
// 규약: `var api`를 선언하지 말고 인자 `api`에 직접 훅을 정의.
// Base 훅은 `Base.<hook>`으로 호출 가능 (프로토타입 체이닝).

// before_create: Base 검증 후, pinned 필드 추가 (기본 false)
api.before_create = function (payload) {
    var validated = Base.before_create(payload); // Base 검증 재사용
    if (typeof validated.pinned === "undefined") {
        validated.pinned = false;
    }
    return validated;
};

// before_delete: 공지사항은 삭제 불가 (정책)
api.before_delete = function (post) {
    throw new Error(JSON.stringify({ code: "SJ1401", message: "공지사항은 삭제할 수 없습니다." }));
};
