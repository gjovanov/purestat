use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use purestat_db::models::OrgRole;
use serde::{Deserialize, Serialize};

use crate::error::ApiError;
use crate::extractors::auth::AuthUser;
use crate::routes::org::{ensure_admin, ensure_member, parse_oid};
use crate::state::AppState;

#[derive(Deserialize)]
pub struct UpdateRoleRequest {
    pub role: OrgRole,
}

#[derive(Serialize)]
pub struct MemberResponse {
    pub id: String,
    pub user_id: String,
    pub email: String,
    pub username: String,
    pub display_name: String,
    pub role: String,
}

pub async fn list(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(org_id): Path<String>,
) -> Result<Json<Vec<MemberResponse>>, ApiError> {
    let org_oid = parse_oid(&org_id)?;
    ensure_member(&state, org_oid, auth.user_id).await?;

    let members = state.org_members.find_by_org(org_oid).await?;
    let mut responses = Vec::new();

    for m in &members {
        if let Ok(user) = state.users.base.find_by_id(m.user_id).await {
            responses.push(MemberResponse {
                id: m.id.map(|id| id.to_hex()).unwrap_or_default(),
                user_id: m.user_id.to_hex(),
                email: user.email,
                username: user.username,
                display_name: user.display_name,
                role: serde_json::to_string(&m.role)
                    .unwrap_or_default()
                    .trim_matches('"')
                    .to_string(),
            });
        }
    }

    Ok(Json(responses))
}

pub async fn update_role(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((org_id, member_id)): Path<(String, String)>,
    Json(body): Json<UpdateRoleRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let org_oid = parse_oid(&org_id)?;
    let member_oid = parse_oid(&member_id)?;
    ensure_admin(&state, org_oid, auth.user_id).await?;

    // Cannot change owner role
    if body.role == OrgRole::Owner {
        return Err(ApiError::BadRequest(
            "Cannot assign owner role".to_string(),
        ));
    }

    state
        .org_members
        .update_role(member_oid, body.role)
        .await?;

    Ok(Json(serde_json::json!({ "status": "updated" })))
}

pub async fn remove(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((org_id, member_id)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    let org_oid = parse_oid(&org_id)?;
    let member_oid = parse_oid(&member_id)?;
    ensure_admin(&state, org_oid, auth.user_id).await?;

    // Cannot remove the owner
    let member = state
        .org_members
        .base
        .find_one(bson::doc! { "_id": member_oid })
        .await?
        .ok_or(ApiError::NotFound("Member not found".to_string()))?;

    if member.role == OrgRole::Owner {
        return Err(ApiError::Forbidden(
            "Cannot remove the organization owner".to_string(),
        ));
    }

    state.org_members.remove(member_oid).await?;
    Ok(StatusCode::NO_CONTENT)
}
