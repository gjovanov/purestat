use axum::extract::FromRef;
use axum::http::header;
use axum::http::request::Parts;
use bson::oid::ObjectId;
use purestat_services::auth::Claims;

use crate::error::ApiError;
use crate::state::AppState;

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: ObjectId,
    pub email: String,
    pub username: String,
    pub claims: Claims,
}

impl<S> axum::extract::FromRequestParts<S> for AuthUser
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let app_state = AppState::from_ref(state);

        // 1. Try Bearer token from Authorization header
        // 2. Fall back to access_token cookie
        let token = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .map(|s| s.to_string())
            .or_else(|| extract_cookie_value(parts, "access_token"))
            .ok_or_else(|| {
                ApiError::Unauthorized("No token provided".to_string())
            })?;

        let claims = app_state.auth.verify_access_token(&token)?;
        let user_id = ObjectId::parse_str(&claims.sub).map_err(|_| {
            ApiError::Unauthorized("Invalid user ID in token".to_string())
        })?;

        Ok(AuthUser {
            user_id,
            email: claims.email.clone(),
            username: claims.username.clone(),
            claims,
        })
    }
}

fn extract_cookie_value(parts: &Parts, name: &str) -> Option<String> {
    parts
        .headers
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|cookies| {
            cookies.split(';').find_map(|cookie| {
                let cookie = cookie.trim();
                if let Some(value) = cookie.strip_prefix(&format!("{name}=")) {
                    Some(value.to_string())
                } else {
                    None
                }
            })
        })
}
