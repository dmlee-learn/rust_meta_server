//! JWT 인증 미들웨어 (Phase 14).
//!
//! `Authorization: Bearer <token>` 헤더에서 JWT 토큰 검증.
//! 도메인별 권한 체크: `{"sub": "user", "domains": ["erp", "blog"]}`.

use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
    Json,
};
use jsonwebtoken::{DecodingKey, Validation, Algorithm};
use serde::{Deserialize, Serialize};

use crate::http::AppState;

/// JWT 클레임.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    /// 허용된 도메인 목록. `None` = 전부 허용.
    #[serde(default)]
    pub domains: Option<Vec<String>>,
    pub exp: i64,
}

/// JWT 설정.
#[derive(Debug, Clone)]
pub struct JwtConfig {
    pub secret: String,
    /// 인증을 완전히 비활성화 (개발용).
    pub disabled: bool,
}

impl JwtConfig {
    /// 환경변수에서 JWT 설정 로드.
    pub fn from_env() -> Self {
        let secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| "dev-secret-change-me".into());
        let disabled = std::env::var("JWT_DISABLED")
            .map(|v| v == "true")
            .unwrap_or(false);
        Self { secret, disabled }
    }
}

/// Bearer 토큰 추출 + 검증.
pub fn verify_token(token: &str, secret: &str) -> Result<Claims, String> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;

    let token_data: jsonwebtoken::TokenData<Claims> = jsonwebtoken::decode(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map_err(|e| format!("JWT 검증 실패: {e}"))?;

    Ok(token_data.claims)
}

/// Phase 19: JWT token decode (env secret 사용).
pub fn decode_token_claims(token: &str) -> Result<Claims, String> {
    let secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| "dev-secret-change-me-in-production".to_string());
    verify_token(token, &secret)
}

/// JWT 인증 미들웨어.
///
/// `/health`, `/metrics`, `/api/engine/info`는 제외.
/// 나머지 엔드포인트는 `Authorization: Bearer <token>` 필요.
pub async fn jwt_middleware(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let path = request.uri().path().to_string();

    // 인증 불필요 경로
    if path == "/health" || path == "/metrics" || path.starts_with("/api/engine") || path.starts_with("/admin") || path.starts_with("/api/admin") {
        return Ok(next.run(request).await);
    }

    // JWT 비활성화 (개발용)
    if state.jwt_disabled {
        return Ok(next.run(request).await);
    }

    // Authorization 헤더 추출
    let auth_header = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if !auth_header.starts_with("Bearer ") {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let token = &auth_header[7..];
    let secret = &state.jwt_secret;

    match verify_token(token, secret) {
        Ok(claims) => {
            // 도메인 권한 체크
            let domain = extract_domain_from_path(&path);
            if let Some(domain) = domain {
                if let Some(allowed) = &claims.domains {
                    if !allowed.contains(&domain.to_string()) {
                        return Err(StatusCode::FORBIDDEN);
                    }
                }
            }
            Ok(next.run(request).await)
        }
        Err(_) => Err(StatusCode::UNAUTHORIZED),
    }
}

/// URL 경로에서 도메인 추출.
fn extract_domain_from_path(path: &str) -> Option<&str> {
    if path.starts_with("/api/board") {
        Some("board")
    } else if path.starts_with("/api/erp") {
        Some("erp")
    } else if path.starts_with("/api/blog") {
        Some("blog")
    } else if path.starts_with("/api/shop") {
        Some("shop")
    } else if path.starts_with("/ws") {
        None // WebSocket은 별도 처리
    } else {
        None
    }
}

/// Phase 25: Admin API 인증 미들웨어.
///
/// `ADMIN_TOKEN` 환경변수 설정 시 `/api/admin/*`에 `X-Admin-Token` 헤더 요구.
/// 미설정 시 pass through (개발용).
pub async fn admin_auth_middleware(
    request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path().to_string();

    // /api/admin/* 경로에만 적용
    if !path.starts_with("/api/admin") {
        return next.run(request).await;
    }

    // ADMIN_TOKEN 미설정 → pass through (개발용)
    let admin_token = match std::env::var("ADMIN_TOKEN") {
        Ok(t) if !t.is_empty() => Some(t),
        _ => None,
    };

    match admin_token {
        None => next.run(request).await,
        Some(expected) => {
            let provided = request
                .headers()
                .get("x-admin-token")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_string();

            if provided == expected {
                next.run(request).await
            } else {
                axum::response::Response::builder()
                    .status(axum::http::StatusCode::UNAUTHORIZED)
                    .header(axum::http::header::CONTENT_TYPE, "application/json")
                    .body(
                        axum::body::Body::from(
                            serde_json::json!({
                                "code": "CR6001",
                                "message": "Unauthorized admin access"
                            })
                            .to_string(),
                        ),
                    )
                    .unwrap()
            }
        }
    }
}

/// Phase 25: CORS 미들웨어.
///
/// `CORS_ORIGINS` 환경변수에서 허용된 origin 목록 로드.
/// 미설정 시 `*` (전부 허용, 개발용).
pub async fn cors_middleware(
    request: Request,
    next: Next,
) -> Response {
    let cors_origins = std::env::var("CORS_ORIGINS").unwrap_or_default();

    // Preflight request (OPTIONS)
    if request.method() == axum::http::Method::OPTIONS {
        let origin = request
            .headers()
            .get("origin")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("*")
            .to_string();

        let allow_origin = if cors_origins.is_empty() {
            "*".to_string()
        } else {
            let origins: Vec<&str> = cors_origins.split(',').map(|s| s.trim()).collect();
            if origins.contains(&origin.as_str()) {
                origin
            } else {
                "*".to_string()
            }
        };

        return axum::response::Response::builder()
            .status(axum::http::StatusCode::NO_CONTENT)
            .header("access-control-allow-origin", &allow_origin)
            .header("access-control-allow-methods", "GET, POST, PUT, DELETE, OPTIONS")
            .header("access-control-allow-headers", "Content-Type, Authorization, X-Admin-Token")
            .header("access-control-max-age", "86400")
            .body(axum::body::Body::empty())
            .unwrap();
    }

    // Actual request
    let cors_origins2 = std::env::var("CORS_ORIGINS").unwrap_or_default();
    let origin = request
        .headers()
        .get("origin")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("*")
        .to_string();

    let allow_origin = if cors_origins2.is_empty() {
        "*".to_string()
    } else {
        let origins: Vec<&str> = cors_origins2.split(',').map(|s| s.trim()).collect();
        if origins.contains(&origin.as_str()) {
            origin
        } else {
            "*".to_string()
        }
    };

    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert("access-control-allow-origin", allow_origin.parse().unwrap());
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{EncodingKey, Header};
    use std::time::SystemTime;

    /// 유효한 JWT 생성.
    pub fn create_test_token(secret: &str, sub: &str, domains: Option<Vec<&str>>) -> String {
        let exp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
            + 3600;

        let claims = Claims {
            sub: sub.to_string(),
            domains: domains.map(|d| d.iter().map(|s| s.to_string()).collect()),
            exp,
        };

        jsonwebtoken::encode(
            &Header::new(Algorithm::HS256),
            &claims,
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .unwrap()
    }

    #[test]
    fn valid_token_passes() {
        let token = create_test_token("test-secret", "user1", Some(vec!["erp", "blog"]));
        let claims = verify_token(&token, "test-secret").unwrap();
        assert_eq!(claims.sub, "user1");
        assert!(claims.domains.is_some());
    }

    #[test]
    fn wrong_secret_fails() {
        let token = create_test_token("test-secret", "user1", None);
        let result = verify_token(&token, "wrong-secret");
        assert!(result.is_err());
    }

    #[test]
    fn expired_token_fails() {
        let exp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
            - 3600; // 1시간 전

        let claims = Claims {
            sub: "user1".to_string(),
            domains: None,
            exp,
        };

        let token = jsonwebtoken::encode(
            &Header::new(Algorithm::HS256),
            &claims,
            &EncodingKey::from_secret(b"test-secret"),
        )
        .unwrap();

        let result = verify_token(&token, "test-secret");
        assert!(result.is_err(), "만료된 토큰은 실패해야 함");
    }
}
