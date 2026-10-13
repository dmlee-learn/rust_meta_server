//! Phase 11: SDK + 추가 도메인 모듈 테스트.
//!
//! - SDK 함수 (rust_calculate_base_tax, rust_calculate_discount, rust_hash_token, rust_validate_uuid)
//! - 추가 도메인 모듈 (ERP, Blog, Shop) Base + Extension 바인딩

use meta_server::engine::{Engine, ThrottleConfig};
use meta_server::sdk::{math, utils};

fn test_engine() -> Engine {
    Engine::new(ThrottleConfig {
        memory_limit: 16 * 1024 * 1024,
        max_stack: 0,
        cpu_timeout_ms: 1500,
    })
    .unwrap()
}

/// SDK 계산 함수 단위 테스트.
#[test]
fn sdk_math_functions() {
    let result = math::rust_calculate_base_tax(10000.0, 0.10);
    assert!((result - 11000.0).abs() < f64::EPSILON);

    let nan = math::rust_calculate_base_tax(-100.0, 0.10);
    assert!(nan.is_nan());

    let nan2 = math::rust_calculate_base_tax(1000.0, 1.5);
    assert!(nan2.is_nan());

    let result = math::rust_calculate_discount(10000.0, 0.20);
    assert!((result - 8000.0).abs() < f64::EPSILON);

    let nan = math::rust_calculate_discount(-100.0, 0.20);
    assert!(nan.is_nan());
}

/// SDK 유틸 함수 단위 테스트.
#[test]
fn sdk_utils_functions() {
    let hash = utils::rust_hash_token("abc".into(), "secret".into());
    assert_eq!(hash.len(), 16);
    assert!(!hash.is_empty());

    let empty = utils::rust_hash_token("".into(), "s".into());
    assert!(empty.is_empty());

    assert!(utils::rust_validate_uuid("550e8400-e29b-41d4-a716-446655440000".into()));
    assert!(!utils::rust_validate_uuid("not-a-uuid".into()));
    assert!(!utils::rust_validate_uuid("".into()));
}

/// SDK 함수가 JS 샌드박스에 바인딩되어 있는지 확인.
#[tokio::test]
async fn sdk_bound_to_sandbox() {
    let engine = test_engine();
    let sb = engine.create_sandbox().unwrap();

    // rust_calculate_base_tax: 10000 * 1.10 = 11000
    let result: i64 = sb.eval_i64("rust_calculate_base_tax(10000, 0.10)").unwrap();
    assert_eq!(result, 11000, "부가세 10%%: 10000→11000");

    // rust_calculate_discount: 10000 * 0.80 = 8000
    let result: i64 = sb.eval_i64("rust_calculate_discount(10000, 0.20)").unwrap();
    assert_eq!(result, 8000, "할인 20%%: 10000→8000");

    // rust_hash_token
    let hash = sb.eval_value_as_string("rust_hash_token('test', 'secret')").unwrap();
    assert!(hash.is_some(), "해시 반환됨");
    assert_eq!(hash.unwrap().len(), 16, "해시 길이 16");

    // rust_validate_uuid
    let result: i64 = sb
        .eval_i64("rust_validate_uuid('550e8400-e29b-41d4-a716-446655440000') ? 1 : 0")
        .unwrap();
    assert_eq!(result, 1, "유효 UUID → true");

    let result: i64 = sb
        .eval_i64("rust_validate_uuid('invalid') ? 1 : 0")
        .unwrap();
    assert_eq!(result, 0, "무효 UUID → false");
}

/// ERP Base + Extension 모듈 로드 테스트.
#[tokio::test]
async fn erp_base_extension_load() {
    let engine = test_engine();
    let sb = engine.create_sandbox().unwrap();

    sb.eval_void(
        r#"
        var Base = (function() {
            var api = {};
            api.before_process = function(payload) {
                if (!payload.workflow_id) {
                    throw new Error(JSON.stringify({code: "EJ1201", message: "워크플로우 ID 필수"}));
                }
                return payload;
            };
            api.after_process = function(event) {
                return event;
            };
            return api;
        })();
        var Extension = (function() {
            var proto = Object.create(Base);
            (function(api) {
                api.before_process = function(payload) {
                    var validated = Base.before_process(payload);
                    if (payload.quantity <= 0) {
                        throw new Error(JSON.stringify({code: "EJ1202", message: "수량은 1 이상"}));
                    }
                    return validated;
                };
                api.before_delete = function(post) {
                    throw new Error(JSON.stringify({code: "EJ1401", message: "발주서 삭제 불가"}));
                };
            })(proto);
            return proto;
        })();
        "#,
    )
    .unwrap();

    // 유효한 발주서
    let result: i64 = sb
        .eval_i64(
            r#"
            try {
                Extension.before_process({workflow_id: "WF001", quantity: 10});
                1;
            } catch(e) { 0; }
            "#,
        )
        .unwrap();
    assert_eq!(result, 1, "유효한 발주서 before_process 통과");

    // 수량 0 → EJ1202
    let result: i64 = sb
        .eval_i64(
            r#"
            try {
                Extension.before_process({workflow_id: "WF001", quantity: 0});
                0;
            } catch(e) {
                var obj = JSON.parse(e.message);
                obj.code === "EJ1202" ? 1 : 0;
            }
            "#,
        )
        .unwrap();
    assert_eq!(result, 1, "수량 0 → EJ1202");

    // 발주서 삭제 → EJ1401
    let result: i64 = sb
        .eval_i64(
            r#"
            try {
                Extension.before_delete({});
                0;
            } catch(e) {
                var obj = JSON.parse(e.message);
                obj.code === "EJ1401" ? 1 : 0;
            }
            "#,
        )
        .unwrap();
    assert_eq!(result, 1, "발주서 삭제 → EJ1401");
}

/// Shop Base에서 SDK 함수 호출 테스트.
#[tokio::test]
async fn shop_base_sdk_call() {
    let engine = test_engine();
    let sb = engine.create_sandbox().unwrap();

    sb.eval_void(
        r#"
        var Base = (function() {
            var api = {};
            api.before_order = function(payload) {
                if (payload.price < 0) {
                    throw new Error(JSON.stringify({code: "SJ1401", message: "가격 0 이상"}));
                }
                var taxedPrice = rust_calculate_base_tax(payload.price, 0.10);
                if (isNaN(taxedPrice)) {
                    throw new Error(JSON.stringify({code: "SJ1401", message: "세전 계산 실패"}));
                }
                payload.taxed_price = taxedPrice;
                return payload;
            };
            return api;
        })();
        "#,
    )
    .unwrap();

    // 유효한 주문: 10000 + 10% 부가세 = 11000
    let result: i64 = sb
        .eval_i64("Base.before_order({price: 10000}).taxed_price")
        .unwrap();
    assert_eq!(result, 11000, "Shop Base: 10000 + 10%% 부가세 = 11000");

    // 음수 가격 → SJ1401
    let result: i64 = sb
        .eval_i64(
            r#"
            try {
                Base.before_order({price: -100});
                0;
            } catch(e) {
                var obj = JSON.parse(e.message);
                obj.code === "SJ1401" ? 1 : 0;
            }
            "#,
        )
        .unwrap();
    assert_eq!(result, 1, "음수 가격 → SJ1401");
}

/// Blog Base + Extension 모듈 로드 테스트.
#[tokio::test]
async fn blog_base_extension_load() {
    let engine = test_engine();
    let sb = engine.create_sandbox().unwrap();

    sb.eval_void(
        r#"
        var Base = (function() {
            var api = {};
            api.before_publish = function(payload) {
                if (!payload.category) {
                    throw new Error(JSON.stringify({code: "BJ1201", message: "카테고리 필수"}));
                }
                if (!payload.title) {
                    throw new Error(JSON.stringify({code: "BJ1202", message: "제목 필수"}));
                }
                return payload;
            };
            api.after_publish = function(event) {
                event.slug = event.title.toLowerCase().replace(/[^a-z0-9]+/g, "-");
                return event;
            };
            return api;
        })();
        var Extension = (function() {
            var proto = Object.create(Base);
            (function(api) {
                api.before_publish = function(payload) {
                    var validated = Base.before_publish(payload);
                    if (typeof validated.languages === "undefined") {
                        validated.languages = ["general"];
                    }
                    return validated;
                };
            })(proto);
            return proto;
        })();
        "#,
    )
    .unwrap();

    // 유효한 블로그 글
    let result: i64 = sb
        .eval_i64(
            r#"
            try {
                Extension.before_publish({title: "Hello World", category: "tech", body: "test"});
                1;
            } catch(e) { 0; }
            "#,
        )
        .unwrap();
    assert_eq!(result, 1, "유효한 블로그 글 before_publish 통과");

    // 카테고리 없음 → BJ1201
    let result: i64 = sb
        .eval_i64(
            r#"
            try {
                Extension.before_publish({title: "Test", body: "test"});
                0;
            } catch(e) {
                var obj = JSON.parse(e.message);
                obj.code === "BJ1201" ? 1 : 0;
            }
            "#,
        )
        .unwrap();
    assert_eq!(result, 1, "카테고리 없음 → BJ1201");
}
