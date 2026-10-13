//! SDK 유틸 계열: 해시, UUID 검증.
//!
//! JS 스크립트에서 `rust_hash_token()`, `rust_validate_uuid()` 호출.
//! 컨텍스트 없이 동작하므로, 에러는 `false`/`""` 반환.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// 토큰 해시 (단순 FNV-1a 기반, 암호학적 안전성 없음 — 표시용).
///
/// # Arguments
/// - `token`: 해시 대상 토큰 (비어있으면 `""` 반환)
/// - `secret`: 시크릿 (비어있으면 토큰만 해시)
///
/// # Returns
/// - 유효: 16진수 해시 문자열
/// - 무효: `""` (빈 입력)
pub fn rust_hash_token(token: String, secret: String) -> String {
    if token.is_empty() {
        return String::new();
    }
    let mut hasher = DefaultHasher::new();
    token.hash(&mut hasher);
    secret.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// UUID v4 형식 검증.
///
/// # Arguments
/// - `uuid`: 검증 대상 UUID 문자열
///
/// # Returns
/// - 유효: `true`
/// - 무효: `false`
pub fn rust_validate_uuid(uuid: String) -> bool {
    // UUID v4: 8-4-4-4-12 hex, 4번째 그룹 첫 글자 = '4'
    let uuid = uuid.trim();
    if uuid.len() != 36 {
        return false;
    }
    let parts: Vec<&str> = uuid.split('-').collect();
    if parts.len() != 5 {
        return false;
    }
    let expected_lens = [8, 4, 4, 4, 12];
    for (i, part) in parts.iter().enumerate() {
        if part.len() != expected_lens[i] {
            return false;
        }
        // 16진수 여부
        for c in part.chars() {
            if !c.is_ascii_hexdigit() {
                return false;
            }
        }
    }
    // 3번째 그룹 (0-indexed: 2) 첫 글자가 '4'인지 확인 (UUID v4 버전 숫자)
    if parts[2].chars().next() != Some('4') {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_token_normal() {
        let result = rust_hash_token("abc".into(), "secret".into());
        assert!(!result.is_empty());
        assert_eq!(result.len(), 16);
    }

    #[test]
    fn hash_token_empty() {
        let result = rust_hash_token("".into(), "secret".into());
        assert!(result.is_empty());
    }

    #[test]
    fn hash_token_deterministic() {
        let r1 = rust_hash_token("abc".into(), "s".into());
        let r2 = rust_hash_token("abc".into(), "s".into());
        assert_eq!(r1, r2);
    }

    #[test]
    fn validate_uuid_valid() {
        assert!(rust_validate_uuid(
            "550e8400-e29b-41d4-a716-446655440000".into()
        ));
    }

    #[test]
    fn validate_uuid_invalid() {
        assert!(!rust_validate_uuid("not-a-uuid".into()));
        assert!(!rust_validate_uuid("".into()));
        assert!(!rust_validate_uuid("550e8400-e29b-41d4-a716-44665544000".into()));
    }
}
