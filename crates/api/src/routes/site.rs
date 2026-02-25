use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::error::ApiError;
use crate::extractors::auth::AuthUser;
use crate::routes::org::{ensure_admin, ensure_member, parse_oid};
use crate::state::AppState;

#[derive(Deserialize)]
pub struct CreateSiteRequest {
    pub domain: String,
    pub name: String,
    pub timezone: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateSiteRequest {
    pub name: Option<String>,
    pub timezone: Option<String>,
    pub is_public: Option<bool>,
    pub allowed_hostnames: Option<Vec<String>>,
}

#[derive(Serialize)]
pub struct SiteResponse {
    pub id: String,
    pub org_id: String,
    pub domain: String,
    pub name: String,
    pub timezone: String,
    pub is_public: bool,
    pub allowed_hostnames: Vec<String>,
}

pub async fn list(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(org_id): Path<String>,
) -> Result<Json<Vec<SiteResponse>>, ApiError> {
    let org_oid = parse_oid(&org_id)?;
    ensure_member(&state, org_oid, auth.user_id).await?;

    let sites = state.sites.find_by_org(org_oid).await?;
    Ok(Json(sites.iter().map(site_to_response).collect()))
}

pub async fn create(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(org_id): Path<String>,
    Json(body): Json<CreateSiteRequest>,
) -> Result<(StatusCode, Json<SiteResponse>), ApiError> {
    let org_oid = parse_oid(&org_id)?;
    ensure_admin(&state, org_oid, auth.user_id).await?;

    // Check plan limits
    let org = state.orgs.base.find_by_id(org_oid).await?;
    let site_count = state.sites.count_by_org(org_oid).await?;
    if site_count >= org.limits.max_sites as u64 {
        return Err(ApiError::Forbidden(
            "Site limit reached for your plan".to_string(),
        ));
    }

    let site = state
        .sites
        .create(org_oid, body.domain, body.name, body.timezone)
        .await?;

    Ok((StatusCode::CREATED, Json(site_to_response(&site))))
}

pub async fn get(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((org_id, site_id)): Path<(String, String)>,
) -> Result<Json<SiteResponse>, ApiError> {
    let org_oid = parse_oid(&org_id)?;
    let site_oid = parse_oid(&site_id)?;
    ensure_member(&state, org_oid, auth.user_id).await?;

    let site = state.sites.base.find_one(
        bson::doc! { "_id": site_oid, "org_id": org_oid },
    ).await?
    .ok_or(ApiError::NotFound("Site not found".to_string()))?;

    Ok(Json(site_to_response(&site)))
}

pub async fn update(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((org_id, site_id)): Path<(String, String)>,
    Json(body): Json<UpdateSiteRequest>,
) -> Result<Json<SiteResponse>, ApiError> {
    let org_oid = parse_oid(&org_id)?;
    let site_oid = parse_oid(&site_id)?;
    ensure_admin(&state, org_oid, auth.user_id).await?;

    let site = state
        .sites
        .update(
            site_oid,
            body.name,
            body.timezone,
            body.is_public,
            body.allowed_hostnames,
        )
        .await?;

    Ok(Json(site_to_response(&site)))
}

pub async fn delete(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((org_id, site_id)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    let org_oid = parse_oid(&org_id)?;
    let site_oid = parse_oid(&site_id)?;
    ensure_admin(&state, org_oid, auth.user_id).await?;

    state.goals.delete_all_for_site(site_oid).await?;
    state.sites.delete(site_oid).await?;
    Ok(StatusCode::NO_CONTENT)
}

fn site_to_response(site: &purestat_db::models::Site) -> SiteResponse {
    SiteResponse {
        id: site.id.map(|id| id.to_hex()).unwrap_or_default(),
        org_id: site.org_id.to_hex(),
        domain: site.domain.clone(),
        name: site.name.clone(),
        timezone: site.timezone.clone(),
        is_public: site.is_public,
        allowed_hostnames: site.allowed_hostnames.clone(),
    }
}
