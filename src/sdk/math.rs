//! SDK 계산 계열: 부가세, 할인 계산.
//!
//! JS 스크립트에서 `rust_calculate_base_tax()`, `rust_calculate_discount()` 호출.
//!
//! # 에러 처리 방식
//! - 컨텍스트 없이 동작하므로, 에러 시 `NaN` 반환 + JS에서 `isNaN()` 확인.
//! - 또는 JS에서 try/catch로 rquickjs 예외를 잡음.
//! - panic하지 않으며, 유효한 입력만 처리.

/// 원가 + 부가세 계산.
///
/// # Arguments
/// - `amount`: 원가 (음수 불가 → NaN 반환)
/// - `rate`: 부가세율 (0.0 ~ 1.0, 유효하지 않으면 NaN 반환)
///
/// # Returns
/// - 유효: `amount * (1.0 + rate)`
/// - 무효: `f64::NAN` (JS에서 `isNaN()`으로 확인)
pub fn rust_calculate_base_tax(amount: f64, rate: f64) -> f64 {
    if amount < 0.0 || rate < 0.0 || rate > 1.0 {
        return f64::NAN;
    }
    amount * (1.0 + rate)
}

/// 할인율 적용.
///
/// # Arguments
/// - `price`: 상품 가격 (음수 불가 → NaN 반환)
/// - `discount_pct`: 할인율 (0.0 ~ 1.0, 유효하지 않으면 NaN 반환)
///
/// # Returns
/// - 유효: `price * (1.0 - discount_pct)`
/// - 무효: `f64::NAN`
pub fn rust_calculate_discount(price: f64, discount_pct: f64) -> f64 {
    if price < 0.0 || discount_pct < 0.0 || discount_pct > 1.0 {
        return f64::NAN;
    }
    price * (1.0 - discount_pct)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_tax_normal() {
        let result = rust_calculate_base_tax(10000.0, 0.10);
        assert!((result - 11000.0).abs() < f64::EPSILON);
    }

    #[test]
    fn base_tax_negative_amount() {
        let result = rust_calculate_base_tax(-100.0, 0.10);
        assert!(result.is_nan());
    }

    #[test]
    fn base_tax_invalid_rate() {
        let result = rust_calculate_base_tax(1000.0, 1.5);
        assert!(result.is_nan());
    }

    #[test]
    fn discount_normal() {
        let result = rust_calculate_discount(10000.0, 0.20);
        assert!((result - 8000.0).abs() < f64::EPSILON);
    }

    #[test]
    fn discount_negative_price() {
        let result = rust_calculate_discount(-100.0, 0.20);
        assert!(result.is_nan());
    }

    #[test]
    fn discount_valid_pct() {
        let result = rust_calculate_discount(1000.0, 0.5);
        assert!((result - 500.0).abs() < f64::EPSILON);
    }
}
