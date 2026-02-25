use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::error::ApiError;
use crate::extractors::auth::AuthUser;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub username: String,
    pub password: String,
    pub display_name: Option<String>,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct AuthResponse {
    pub user: UserResponse,
    pub access_token: String,
}

#[derive(Serialize)]
pub struct UserResponse {
    pub id: String,
    pub email: String,
    pub username: String,
    pub display_name: String,
    pub avatar: Option<String>,
    pub locale: String,
}

#[derive(Deserialize)]
pub struct UpdateProfileRequest {
    pub display_name: Option<String>,
    pub avatar: Option<String>,
    pub locale: Option<String>,
}

pub async fn register(
    State(state): State<AppState>,
    Json(body): Json<RegisterRequest>,
) -> Result<(StatusCode, HeaderMap, Json<AuthResponse>), ApiError> {
    let password_hash = state.auth.hash_password(&body.password)?;
    let display_name = body
        .display_name
        .unwrap_or_else(|| body.username.clone());

    let user = state
        .users
        .create(body.email, body.username, display_name, Some(password_hash))
        .await?;

    let user_id = user.id.unwrap().to_hex();
    let access_token =
        state
            .auth
            .generate_access_token(&user_id, &user.email, &user.username)?;
    let refresh_token =
        state
            .auth
            .generate_refresh_token(&user_id, &user.email, &user.username)?;

    let mut headers = HeaderMap::new();
    set_auth_cookies(&mut headers, &access_token, &refresh_token);

    Ok((
        StatusCode::CREATED,
        headers,
        Json(AuthResponse {
            user: user_to_response(&user),
            access_token,
        }),
    ))
}

pub async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> Result<(HeaderMap, Json<AuthResponse>), ApiError> {
    let user = state
        .users
        .find_by_email(&body.email)
        .await
        .map_err(|_| ApiError::Unauthorized("Invalid credentials".to_string()))?;

    let password_hash = user
        .password_hash
        .as_ref()
        .ok_or_else(|| ApiError::Unauthorized("No password set".to_string()))?;

    let valid = state.auth.verify_password(&body.password, password_hash)?;
    if !valid {
        return Err(ApiError::Unauthorized(
            "Invalid credentials".to_string(),
        ));
    }

    let user_id = user.id.unwrap().to_hex();
    let access_token =
        state
            .auth
            .generate_access_token(&user_id, &user.email, &user.username)?;
    let refresh_token =
        state
            .auth
            .generate_refresh_token(&user_id, &user.email, &user.username)?;

    let mut headers = HeaderMap::new();
    set_auth_cookies(&mut headers, &access_token, &refresh_token);

    Ok((
        headers,
        Json(AuthResponse {
            user: user_to_response(&user),
            access_token,
        }),
    ))
}

pub async fn logout() -> (HeaderMap, StatusCode) {
    let mut headers = HeaderMap::new();
    headers.append(
        axum::http::header::SET_COOKIE,
        HeaderValue::from_static(
            "access_token=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0",
        ),
    );
    headers.append(
        axum::http::header::SET_COOKIE,
        HeaderValue::from_static(
            "refresh_token=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0",
        ),
    );
    (headers, StatusCode::OK)
}

pub async fn me(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<UserResponse>, ApiError> {
    let user = state.users.base.find_by_id(auth.user_id).await?;
    Ok(Json(user_to_response(&user)))
}

pub async fn update_me(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<UpdateProfileRequest>,
) -> Result<Json<UserResponse>, ApiError> {
    let user = state
        .users
        .update_profile(auth.user_id, body.display_name, body.avatar, body.locale)
        .await?;
    Ok(Json(user_to_response(&user)))
}

pub async fn refresh(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<(HeaderMap, Json<AuthResponse>), ApiError> {
    let refresh_token = headers
        .get(axum::http::header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|cookies| {
            cookies.split(';').find_map(|c| {
                let c = c.trim();
                c.strip_prefix("refresh_token=").map(|v| v.to_string())
            })
        })
        .ok_or_else(|| {
            ApiError::Unauthorized("No refresh token".to_string())
        })?;

    let claims = state.auth.verify_refresh_token(&refresh_token)?;
    let user_id =
        bson::oid::ObjectId::parse_str(&claims.sub).map_err(|_| {
            ApiError::Unauthorized("Invalid user ID".to_string())
        })?;

    let user = state.users.base.find_by_id(user_id).await?;
    let uid = user.id.unwrap().to_hex();
    let new_access =
        state
            .auth
            .generate_access_token(&uid, &user.email, &user.username)?;
    let new_refresh =
        state
            .auth
            .generate_refresh_token(&uid, &user.email, &user.username)?;

    let mut resp_headers = HeaderMap::new();
    set_auth_cookies(&mut resp_headers, &new_access, &new_refresh);

    Ok((
        resp_headers,
        Json(AuthResponse {
            user: user_to_response(&user),
            access_token: new_access,
        }),
    ))
}

fn set_auth_cookies(headers: &mut HeaderMap, access_token: &str, refresh_token: &str) {
    if let Ok(val) = HeaderValue::from_str(&format!(
        "access_token={access_token}; Path=/; HttpOnly; SameSite=Lax; Max-Age=86400"
    )) {
        headers.append(axum::http::header::SET_COOKIE, val);
    }
    if let Ok(val) = HeaderValue::from_str(&format!(
        "refresh_token={refresh_token}; Path=/; HttpOnly; SameSite=Lax; Max-Age=604800"
    )) {
        headers.append(axum::http::header::SET_COOKIE, val);
    }
}

fn user_to_response(user: &purestat_db::models::User) -> UserResponse {
    UserResponse {
        id: user.id.map(|id| id.to_hex()).unwrap_or_default(),
        email: user.email.clone(),
        username: user.username.clone(),
        display_name: user.display_name.clone(),
        avatar: user.avatar.clone(),
        locale: user.locale.clone(),
    }
}
