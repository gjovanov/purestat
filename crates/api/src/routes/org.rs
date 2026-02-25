use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use bson::oid::ObjectId;
use purestat_db::models::OrgRole;
use serde::{Deserialize, Serialize};

use crate::error::ApiError;
use crate::extractors::auth::AuthUser;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct CreateOrgRequest {
    pub name: String,
    pub slug: String,
}

#[derive(Deserialize)]
pub struct UpdateOrgRequest {
    pub name: Option<String>,
}

#[derive(Serialize)]
pub struct OrgResponse {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub owner_id: String,
    pub plan: String,
    pub limits: LimitsResponse,
    pub usage: UsageResponse,
}

#[derive(Serialize)]
pub struct LimitsResponse {
    pub max_sites: u32,
    pub max_members: u32,
    pub max_pageviews_monthly: u64,
}

#[derive(Serialize)]
pub struct UsageResponse {
    pub current_month_pageviews: u64,
}

pub async fn list(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<OrgResponse>>, ApiError> {
    let memberships = state.org_members.find_by_user(auth.user_id).await?;
    let mut orgs = Vec::new();
    for m in memberships {
        if let Ok(org) = state.orgs.base.find_by_id(m.org_id).await {
            orgs.push(org_to_response(&org));
        }
    }
    Ok(Json(orgs))
}

pub async fn create(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<CreateOrgRequest>,
) -> Result<(StatusCode, Json<OrgResponse>), ApiError> {
    let org = state
        .orgs
        .create(body.name, body.slug, auth.user_id)
        .await?;

    // Add creator as owner member
    state
        .org_members
        .create(org.id.unwrap(), auth.user_id, OrgRole::Owner, None)
        .await?;

    Ok((StatusCode::CREATED, Json(org_to_response(&org))))
}

pub async fn get(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(org_id): Path<String>,
) -> Result<Json<OrgResponse>, ApiError> {
    let org_oid = parse_oid(&org_id)?;
    ensure_member(&state, org_oid, auth.user_id).await?;

    let org = state.orgs.base.find_by_id(org_oid).await?;
    Ok(Json(org_to_response(&org)))
}

pub async fn update(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(org_id): Path<String>,
    Json(body): Json<UpdateOrgRequest>,
) -> Result<Json<OrgResponse>, ApiError> {
    let org_oid = parse_oid(&org_id)?;
    ensure_admin(&state, org_oid, auth.user_id).await?;

    let org = state.orgs.update(org_oid, body.name).await?;
    Ok(Json(org_to_response(&org)))
}

pub async fn delete(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(org_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let org_oid = parse_oid(&org_id)?;
    let org = state.orgs.base.find_by_id(org_oid).await?;

    if org.owner_id != auth.user_id {
        return Err(ApiError::Forbidden(
            "Only the owner can delete an organization".to_string(),
        ));
    }

    state.org_members.remove_all_for_org(org_oid).await?;
    state.orgs.delete(org_oid).await?;
    Ok(StatusCode::NO_CONTENT)
}

// Helpers

pub fn parse_oid(s: &str) -> Result<ObjectId, ApiError> {
    ObjectId::parse_str(s)
        .map_err(|_| ApiError::BadRequest(format!("Invalid ID: {s}")))
}

pub async fn ensure_member(
    state: &AppState,
    org_id: ObjectId,
    user_id: ObjectId,
) -> Result<purestat_db::models::OrgMember, ApiError> {
    state
        .org_members
        .find_membership(org_id, user_id)
        .await
        .map_err(|_| ApiError::Forbidden("Not a member of this organization".to_string()))
}

pub async fn ensure_admin(
    state: &AppState,
    org_id: ObjectId,
    user_id: ObjectId,
) -> Result<purestat_db::models::OrgMember, ApiError> {
    let member = ensure_member(state, org_id, user_id).await?;
    match member.role {
        OrgRole::Owner | OrgRole::Admin => Ok(member),
        _ => Err(ApiError::Forbidden(
            "Admin or owner role required".to_string(),
        )),
    }
}

fn org_to_response(org: &purestat_db::models::Org) -> OrgResponse {
    OrgResponse {
        id: org.id.map(|id| id.to_hex()).unwrap_or_default(),
        name: org.name.clone(),
        slug: org.slug.clone(),
        owner_id: org.owner_id.to_hex(),
        plan: serde_json::to_string(&org.plan).unwrap_or_default().trim_matches('"').to_string(),
        limits: LimitsResponse {
            max_sites: org.limits.max_sites,
            max_members: org.limits.max_members,
            max_pageviews_monthly: org.limits.max_pageviews_monthly,
        },
        usage: UsageResponse {
            current_month_pageviews: org.usage.current_month_pageviews,
        },
    }
}
