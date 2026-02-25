use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::Redirect;
use serde::Deserialize;

use crate::error::ApiError;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct OAuthCallback {
    pub code: String,
    pub state: Option<String>,
}

pub async fn oauth_redirect(
    State(state): State<AppState>,
    Path(provider): Path<String>,
) -> Result<Redirect, ApiError> {
    let config = get_provider_config(&state, &provider)?;

    let auth_url = match provider.as_str() {
        "google" => format!(
            "https://accounts.google.com/o/oauth2/v2/auth?client_id={}&redirect_uri={}&response_type=code&scope=email%20profile&access_type=offline",
            config.client_id,
            urlencoding::encode(&config.redirect_uri)
        ),
        "github" => format!(
            "https://github.com/login/oauth/authorize?client_id={}&redirect_uri={}&scope=user:email",
            config.client_id,
            urlencoding::encode(&config.redirect_uri)
        ),
        "facebook" => format!(
            "https://www.facebook.com/v18.0/dialog/oauth?client_id={}&redirect_uri={}&scope=email",
            config.client_id,
            urlencoding::encode(&config.redirect_uri)
        ),
        "linkedin" => format!(
            "https://www.linkedin.com/oauth/v2/authorization?client_id={}&redirect_uri={}&response_type=code&scope=openid%20profile%20email",
            config.client_id,
            urlencoding::encode(&config.redirect_uri)
        ),
        "microsoft" => format!(
            "https://login.microsoftonline.com/common/oauth2/v2.0/authorize?client_id={}&redirect_uri={}&response_type=code&scope=openid%20profile%20email",
            config.client_id,
            urlencoding::encode(&config.redirect_uri)
        ),
        _ => return Err(ApiError::BadRequest(format!("Unknown provider: {provider}"))),
    };

    Ok(Redirect::temporary(&auth_url))
}

pub async fn oauth_callback(
    State(state): State<AppState>,
    Path(provider): Path<String>,
    Query(params): Query<OAuthCallback>,
) -> Result<(StatusCode, HeaderMap, Redirect), ApiError> {
    let config = get_provider_config(&state, &provider)?;

    // Exchange code for token
    let client = reqwest::Client::new();
    let (token_url, token_params) = match provider.as_str() {
        "google" => (
            "https://oauth2.googleapis.com/token",
            vec![
                ("code", params.code.as_str()),
                ("client_id", &config.client_id),
                ("client_secret", &config.client_secret),
                ("redirect_uri", &config.redirect_uri),
                ("grant_type", "authorization_code"),
            ],
        ),
        "github" => (
            "https://github.com/login/oauth/access_token",
            vec![
                ("code", params.code.as_str()),
                ("client_id", &config.client_id),
                ("client_secret", &config.client_secret),
                ("redirect_uri", &config.redirect_uri),
            ],
        ),
        _ => {
            return Err(ApiError::BadRequest(format!(
                "OAuth callback not implemented for: {provider}"
            )));
        }
    };

    let resp = client
        .post(token_url)
        .header("Accept", "application/json")
        .form(&token_params)
        .send()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let token_data: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let access_token = token_data["access_token"]
        .as_str()
        .ok_or_else(|| ApiError::Internal("No access_token in response".to_string()))?;

    // Get user info
    let (email, name, provider_id, _avatar) =
        fetch_user_info(&client, &provider, access_token).await?;

    // Find or create user
    let user = match state.users.find_by_email(&email).await {
        Ok(existing) => existing,
        Err(_) => {
            let username = email.split('@').next().unwrap_or("user").to_string();
            state
                .users
                .create(email.clone(), username, name, None)
                .await?
        }
    };

    // Add OAuth provider if not already linked
    let has_provider = user
        .oauth_providers
        .iter()
        .any(|p| p.provider == provider && p.provider_id == provider_id);

    if !has_provider {
        state
            .users
            .add_oauth_provider(
                user.id.unwrap(),
                purestat_db::models::OAuthProvider {
                    provider: provider.clone(),
                    provider_id,
                },
            )
            .await?;
    }

    // Generate JWT
    let user_id = user.id.unwrap().to_hex();
    let jwt_access = state
        .auth
        .generate_access_token(&user_id, &user.email, &user.username)?;
    let jwt_refresh = state
        .auth
        .generate_refresh_token(&user_id, &user.email, &user.username)?;

    let mut headers = HeaderMap::new();
    if let Ok(val) = HeaderValue::from_str(&format!(
        "access_token={jwt_access}; Path=/; HttpOnly; SameSite=Lax; Max-Age=86400"
    )) {
        headers.append(axum::http::header::SET_COOKIE, val);
    }
    if let Ok(val) = HeaderValue::from_str(&format!(
        "refresh_token={jwt_refresh}; Path=/; HttpOnly; SameSite=Lax; Max-Age=604800"
    )) {
        headers.append(axum::http::header::SET_COOKIE, val);
    }

    let redirect_url = format!("{}/?token={jwt_access}", state.settings.app.frontend_url);
    Ok((StatusCode::FOUND, headers, Redirect::temporary(&redirect_url)))
}

fn get_provider_config(
    state: &AppState,
    provider: &str,
) -> Result<purestat_config::OAuthProviderSettings, ApiError> {
    let oauth = &state.settings.oauth;
    let config = match provider {
        "google" => oauth.google.clone(),
        "github" => oauth.github.clone(),
        "facebook" => oauth.facebook.clone(),
        "linkedin" => oauth.linkedin.clone(),
        "microsoft" => oauth.microsoft.clone(),
        _ => None,
    };
    config.ok_or_else(|| {
        ApiError::BadRequest(format!("OAuth provider not configured: {provider}"))
    })
}

async fn fetch_user_info(
    client: &reqwest::Client,
    provider: &str,
    access_token: &str,
) -> Result<(String, String, String, Option<String>), ApiError> {
    match provider {
        "google" => {
            let resp: serde_json::Value = client
                .get("https://www.googleapis.com/oauth2/v2/userinfo")
                .bearer_auth(access_token)
                .send()
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?
                .json()
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;

            Ok((
                resp["email"].as_str().unwrap_or("").to_string(),
                resp["name"].as_str().unwrap_or("").to_string(),
                resp["id"].as_str().unwrap_or("").to_string(),
                resp["picture"].as_str().map(|s| s.to_string()),
            ))
        }
        "github" => {
            let resp: serde_json::Value = client
                .get("https://api.github.com/user")
                .bearer_auth(access_token)
                .header("User-Agent", "purestat")
                .send()
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?
                .json()
                .await
                .map_err(|e| ApiError::Internal(e.to_string()))?;

            // GitHub may not return email in user endpoint — fetch from emails API
            let email = if let Some(e) = resp["email"].as_str() {
                e.to_string()
            } else {
                let emails: Vec<serde_json::Value> = client
                    .get("https://api.github.com/user/emails")
                    .bearer_auth(access_token)
                    .header("User-Agent", "purestat")
                    .send()
                    .await
                    .map_err(|e| ApiError::Internal(e.to_string()))?
                    .json()
                    .await
                    .map_err(|e| ApiError::Internal(e.to_string()))?;

                emails
                    .iter()
                    .find(|e| e["primary"].as_bool() == Some(true))
                    .and_then(|e| e["email"].as_str())
                    .unwrap_or("")
                    .to_string()
            };

            Ok((
                email,
                resp["name"]
                    .as_str()
                    .or(resp["login"].as_str())
                    .unwrap_or("")
                    .to_string(),
                resp["id"].to_string(),
                resp["avatar_url"].as_str().map(|s| s.to_string()),
            ))
        }
        _ => Err(ApiError::BadRequest(format!(
            "User info fetch not implemented for: {provider}"
        ))),
    }
}
