use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use bson::DateTime;
use purestat_db::models::{InviteStatus, OrgRole};
use serde::{Deserialize, Serialize};

use crate::error::ApiError;
use crate::extractors::auth::AuthUser;
use crate::routes::org::{ensure_admin, parse_oid};
use crate::state::AppState;

#[derive(Deserialize)]
pub struct CreateInviteRequest {
    pub target_email: Option<String>,
    pub role: Option<OrgRole>,
    pub max_uses: Option<u32>,
    pub expires_in_hours: Option<u64>,
}

#[derive(Serialize)]
pub struct InviteResponse {
    pub id: String,
    pub org_id: String,
    pub code: String,
    pub target_email: Option<String>,
    pub role: String,
    pub max_uses: u32,
    pub use_count: u32,
    pub status: String,
}

pub async fn create(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(org_id): Path<String>,
    Json(body): Json<CreateInviteRequest>,
) -> Result<(StatusCode, Json<InviteResponse>), ApiError> {
    let org_oid = parse_oid(&org_id)?;
    ensure_admin(&state, org_oid, auth.user_id).await?;

    let code = nanoid::nanoid!(12);
    let role = body.role.unwrap_or(OrgRole::Viewer);
    let max_uses = body.max_uses.unwrap_or(1);
    let hours = body.expires_in_hours.unwrap_or(72);
    let expires_at = DateTime::from_millis(
        chrono::Utc::now().timestamp_millis() + (hours as i64 * 3600 * 1000),
    );

    let invite = state
        .invites
        .create(
            org_oid,
            auth.user_id,
            code,
            body.target_email,
            role,
            max_uses,
            expires_at,
        )
        .await?;

    Ok((StatusCode::CREATED, Json(invite_to_response(&invite))))
}

pub async fn list(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(org_id): Path<String>,
) -> Result<Json<Vec<InviteResponse>>, ApiError> {
    let org_oid = parse_oid(&org_id)?;
    ensure_admin(&state, org_oid, auth.user_id).await?;

    let invites = state.invites.find_by_org(org_oid).await?;
    Ok(Json(invites.iter().map(invite_to_response).collect()))
}

pub async fn info(
    State(state): State<AppState>,
    Path(code): Path<String>,
) -> Result<Json<InviteInfoResponse>, ApiError> {
    let invite = state.invites.find_by_code(&code).await?;
    let org = state.orgs.base.find_by_id(invite.org_id).await?;

    Ok(Json(InviteInfoResponse {
        org_name: org.name,
        role: serde_json::to_string(&invite.role)
            .unwrap_or_default()
            .trim_matches('"')
            .to_string(),
        status: serde_json::to_string(&invite.status)
            .unwrap_or_default()
            .trim_matches('"')
            .to_string(),
    }))
}

#[derive(Serialize)]
pub struct InviteInfoResponse {
    pub org_name: String,
    pub role: String,
    pub status: String,
}

pub async fn accept(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(code): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let invite = state.invites.find_by_code(&code).await?;

    if invite.status != InviteStatus::Pending {
        return Err(ApiError::BadRequest("Invite is not pending".to_string()));
    }

    let now_millis = chrono::Utc::now().timestamp_millis();
    let expires_millis = invite.expires_at.timestamp_millis();
    if now_millis > expires_millis {
        state
            .invites
            .update_status(invite.id.unwrap(), InviteStatus::Expired)
            .await?;
        return Err(ApiError::BadRequest("Invite has expired".to_string()));
    }

    if invite.use_count >= invite.max_uses {
        return Err(ApiError::BadRequest(
            "Invite has reached maximum uses".to_string(),
        ));
    }

    // Check if already a member
    if state
        .org_members
        .find_membership(invite.org_id, auth.user_id)
        .await
        .is_ok()
    {
        return Err(ApiError::Conflict(
            "Already a member of this organization".to_string(),
        ));
    }

    // Add as member
    state
        .org_members
        .create(
            invite.org_id,
            auth.user_id,
            invite.role.clone(),
            Some(invite.inviter_id),
        )
        .await?;

    // Increment use count
    state
        .invites
        .increment_use_count(invite.id.unwrap())
        .await?;

    // Mark as accepted if single-use
    if invite.max_uses == 1 {
        state
            .invites
            .update_status(invite.id.unwrap(), InviteStatus::Accepted)
            .await?;
    }

    Ok(Json(serde_json::json!({ "status": "accepted" })))
}

pub async fn revoke(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((org_id, invite_id)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    let org_oid = parse_oid(&org_id)?;
    let invite_oid = parse_oid(&invite_id)?;
    ensure_admin(&state, org_oid, auth.user_id).await?;

    state
        .invites
        .update_status(invite_oid, InviteStatus::Revoked)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

fn invite_to_response(invite: &purestat_db::models::Invite) -> InviteResponse {
    InviteResponse {
        id: invite.id.map(|id| id.to_hex()).unwrap_or_default(),
        org_id: invite.org_id.to_hex(),
        code: invite.code.clone(),
        target_email: invite.target_email.clone(),
        role: serde_json::to_string(&invite.role)
            .unwrap_or_default()
            .trim_matches('"')
            .to_string(),
        max_uses: invite.max_uses,
        use_count: invite.use_count,
        status: serde_json::to_string(&invite.status)
            .unwrap_or_default()
            .trim_matches('"')
            .to_string(),
    }
}
