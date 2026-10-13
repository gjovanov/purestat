use axum::Json;
use axum::extract::{Path, State};
use purestat_services::analytics::query::{QueryError, StatsQuery};
use purestat_services::analytics::scope::SiteScope;
use serde::Deserialize;

use crate::error::ApiError;
use crate::extractors::auth::AuthUser;
use crate::routes::org::{ensure_member, ensure_site_in_org, parse_oid};
use crate::state::AppState;

pub async fn query(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((org_id, site_id)): Path<(String, String)>,
    Json(body): Json<StatsQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let org_oid = parse_oid(&org_id)?;
    let site_oid = parse_oid(&site_id)?;
    ensure_member(&state, org_oid, auth.user_id).await?;
    let site = ensure_site_in_org(&state, org_oid, site_oid).await?;
    tracing::info!(org_id = %org_oid, site_id = %site_oid, user_id = %auth.user_id, "stats query");

    let result = state
        .query
        .query_stats(&SiteScope::for_site(&site), &body)
        .await
        .map_err(|e| match e {
            QueryError::InvalidQuery(msg) => ApiError::BadRequest(msg),
            e => ApiError::Internal(e.to_string()),
        })?;

    Ok(Json(serde_json::to_value(result).unwrap_or_default()))
}

#[derive(Deserialize)]
pub struct MultiSiteStatsQuery {
    pub site_ids: Vec<String>,
    #[serde(flatten)]
    pub query: StatsQuery,
}

/// The scopes of the named sites, each resolved within the org. A site id
/// from another org is a 400, as before.
pub async fn org_site_scopes<'a>(
    state: &AppState,
    org_oid: bson::oid::ObjectId,
    site_ids: impl IntoIterator<Item = &'a str>,
) -> Result<Vec<SiteScope>, ApiError> {
    let org_sites = state.sites.find_by_org(org_oid).await?;
    site_ids
        .into_iter()
        .map(|sid| {
            org_sites
                .iter()
                .find(|s| s.id.is_some_and(|id| id.to_hex() == sid))
                .map(SiteScope::for_site)
                .ok_or_else(|| ApiError::BadRequest(format!("Site {sid} not in org")))
        })
        .collect()
}

pub async fn query_multi(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(org_id): Path<String>,
    Json(body): Json<MultiSiteStatsQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let org_oid = parse_oid(&org_id)?;
    ensure_member(&state, org_oid, auth.user_id).await?;

    if body.site_ids.is_empty() {
        return Err(ApiError::BadRequest("No site IDs provided".to_string()));
    }
    let scopes = org_site_scopes(&state, org_oid, body.site_ids.iter().map(String::as_str)).await?;
    tracing::info!(org_id = %org_oid, sites = scopes.len(), user_id = %auth.user_id, "multi-site stats query");

    let result = state
        .query
        .query_stats_multi(&scopes, &body.query)
        .await
        .map_err(|e| match e {
            QueryError::InvalidQuery(msg) => ApiError::BadRequest(msg),
            e => ApiError::Internal(e.to_string()),
        })?;

    Ok(Json(serde_json::to_value(result).unwrap_or_default()))
}
