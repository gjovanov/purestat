use axum::Json;
use axum::extract::{Path, Query, State};
use purestat_services::analytics::scope::SiteScope;
use serde::Deserialize;

use crate::error::ApiError;
use crate::extractors::auth::AuthUser;
use crate::routes::org::{ensure_member, ensure_site_in_org, parse_oid};
use crate::routes::stats::org_site_scopes;
use crate::state::AppState;

pub async fn current_visitors(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((org_id, site_id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let org_oid = parse_oid(&org_id)?;
    let site_oid = parse_oid(&site_id)?;
    ensure_member(&state, org_oid, auth.user_id).await?;
    let site = ensure_site_in_org(&state, org_oid, site_oid).await?;
    tracing::info!(org_id = %org_oid, site_id = %site_oid, user_id = %auth.user_id, "realtime query");

    let stats = state
        .realtime
        .get_current_visitors(&SiteScope::for_site(&site))
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(serde_json::to_value(stats).unwrap_or_default()))
}

#[derive(Deserialize)]
pub struct MultiSiteQuery {
    pub site_ids: String,
}

pub async fn current_visitors_multi(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(org_id): Path<String>,
    Query(query): Query<MultiSiteQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let org_oid = parse_oid(&org_id)?;
    ensure_member(&state, org_oid, auth.user_id).await?;

    let site_id_strs: Vec<&str> = query.site_ids.split(',').map(|s| s.trim()).collect();
    if site_id_strs.is_empty() || (site_id_strs.len() == 1 && site_id_strs[0].is_empty()) {
        return Err(ApiError::BadRequest("No site IDs provided".to_string()));
    }
    let scopes = org_site_scopes(&state, org_oid, site_id_strs).await?;

    tracing::info!(org_id = %org_oid, sites = scopes.len(), user_id = %auth.user_id, "multi-site realtime query");

    let stats = state
        .realtime
        .get_current_visitors_multi(&scopes)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(serde_json::to_value(stats).unwrap_or_default()))
}
