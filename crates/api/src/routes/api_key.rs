use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::ApiError;
use crate::extractors::auth::AuthUser;
use crate::routes::org::{ensure_admin, parse_oid};
use crate::state::AppState;

#[derive(Deserialize)]
pub struct CreateApiKeyRequest {
    pub name: String,
    pub scopes: Option<Vec<String>>,
}

#[derive(Serialize)]
pub struct ApiKeyResponse {
    pub id: String,
    pub name: String,
    pub key_prefix: String,
    pub scopes: Vec<String>,
    pub created_at: String,
    pub revoked_at: Option<String>,
}

#[derive(Serialize)]
pub struct ApiKeyCreatedResponse {
    pub id: String,
    pub name: String,
    pub key: String,
    pub key_prefix: String,
    pub scopes: Vec<String>,
}

pub async fn list(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((org_id, site_id)): Path<(String, String)>,
) -> Result<Json<Vec<ApiKeyResponse>>, ApiError> {
    let org_oid = parse_oid(&org_id)?;
    let site_oid = parse_oid(&site_id)?;
    crate::routes::org::ensure_member(&state, org_oid, auth.user_id).await?;

    let keys = state.api_keys.find_by_site(site_oid).await?;
    Ok(Json(keys.iter().map(key_to_response).collect()))
}

pub async fn create(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((org_id, site_id)): Path<(String, String)>,
    Json(body): Json<CreateApiKeyRequest>,
) -> Result<(StatusCode, Json<ApiKeyCreatedResponse>), ApiError> {
    let org_oid = parse_oid(&org_id)?;
    let site_oid = parse_oid(&site_id)?;
    ensure_admin(&state, org_oid, auth.user_id).await?;

    // Generate a random API key
    let raw_key = format!("ps_{}", nanoid::nanoid!(32));
    let key_prefix = raw_key[..11].to_string();
    let key_hash = hex::encode(Sha256::digest(raw_key.as_bytes()));

    let scopes = body
        .scopes
        .unwrap_or_else(|| vec!["stats:read".to_string()]);

    let api_key = state
        .api_keys
        .create(site_oid, org_oid, body.name, key_hash, key_prefix.clone(), scopes)
        .await?;

    Ok((
        StatusCode::CREATED,
        Json(ApiKeyCreatedResponse {
            id: api_key.id.map(|id| id.to_hex()).unwrap_or_default(),
            name: api_key.name,
            key: raw_key,
            key_prefix,
            scopes: api_key.scopes,
        }),
    ))
}

pub async fn revoke(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((org_id, _site_id, key_id)): Path<(String, String, String)>,
) -> Result<StatusCode, ApiError> {
    let org_oid = parse_oid(&org_id)?;
    let key_oid = parse_oid(&key_id)?;
    ensure_admin(&state, org_oid, auth.user_id).await?;

    state.api_keys.revoke(key_oid).await?;
    Ok(StatusCode::NO_CONTENT)
}

fn key_to_response(key: &purestat_db::models::ApiKey) -> ApiKeyResponse {
    ApiKeyResponse {
        id: key.id.map(|id| id.to_hex()).unwrap_or_default(),
        name: key.name.clone(),
        key_prefix: key.key_prefix.clone(),
        scopes: key.scopes.clone(),
        created_at: key.created_at.to_string(),
        revoked_at: key.revoked_at.map(|d| d.to_string()),
    }
}
