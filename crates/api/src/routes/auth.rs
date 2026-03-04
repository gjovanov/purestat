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

#[derive(Deserialize)]
pub struct ActivateRequest {
    pub user_id: String,
    pub token: String,
}

#[derive(Serialize)]
pub struct MessageResponse {
    pub message: String,
}

pub async fn register(
    State(state): State<AppState>,
    Json(body): Json<RegisterRequest>,
) -> Result<(StatusCode, Json<MessageResponse>), ApiError> {
    let password_hash = state.auth.hash_password(&body.password)?;
    let display_name = body
        .display_name
        .unwrap_or_else(|| body.username.clone());
    let email = body.email.clone();

    let user = state
        .users
        .create(body.email, body.username, display_name.clone(), Some(password_hash))
        .await?;

    let user_id_oid = user.id.unwrap();
    let user_id = user_id_oid.to_hex();

    // Generate activation code and send email (non-fatal)
    let activation_token = nanoid::nanoid!(7);
    if let Err(e) = state
        .activation_codes
        .create(
            user_id_oid,
            activation_token.clone(),
            state.settings.email.activation_token_ttl_minutes,
        )
        .await
    {
        tracing::warn!("Failed to create activation code: {:?}", e);
    } else if let Some(ref email_svc) = state.email {
        let activation_url = format!(
            "{}/auth/activate?userId={}&token={}",
            state.settings.app.frontend_url, user_id, activation_token
        );
        if let Err(e) = email_svc
            .send_activation(
                &email,
                &display_name,
                &activation_url,
                state.settings.email.activation_token_ttl_minutes,
            )
            .await
        {
            tracing::warn!("Failed to send activation email: {:?}", e);
        }
    }

    Ok((
        StatusCode::CREATED,
        Json(MessageResponse {
            message: "Registration successful. Please check your email to activate your account.".to_string(),
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

    if !user.is_verified {
        return Err(ApiError::Unauthorized(
            "Account not activated. Please check your email for the activation link.".to_string(),
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

pub async fn activate(
    State(state): State<AppState>,
    Json(body): Json<ActivateRequest>,
) -> Result<Json<MessageResponse>, ApiError> {
    let user_id = bson::oid::ObjectId::parse_str(&body.user_id)
        .map_err(|_| ApiError::BadRequest("Invalid user ID".to_string()))?;

    let _code = state
        .activation_codes
        .find_valid(user_id, &body.token)
        .await
        .map_err(|e| ApiError::Internal(format!("Database error: {}", e)))?
        .ok_or_else(|| ApiError::BadRequest("Invalid or expired activation token".to_string()))?;

    state
        .users
        .base
        .update_by_id(user_id, bson::doc! { "$set": { "is_verified": true } })
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to activate: {}", e)))?;

    let _ = state.activation_codes.delete_for_user(user_id).await;

    // Send success email (non-fatal)
    if let Some(ref email_svc) = state.email {
        if let Ok(user) = state.users.base.find_by_id(user_id).await {
            let login_url = format!("{}/auth/login", state.settings.app.frontend_url);
            let _ = email_svc
                .send_activation_success(&user.email, &user.display_name, &login_url)
                .await;
        }
    }

    Ok(Json(MessageResponse {
        message: "Account activated successfully. You can now sign in.".to_string(),
    }))
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
