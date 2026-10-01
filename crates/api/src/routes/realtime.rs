use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;

use crate::error::ApiError;
use crate::extractors::auth::AuthUser;
use crate::routes::org::{ensure_member, ensure_site_in_org, parse_oid};
use crate::state::AppState;

pub async fn current_visitors(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((org_id, site_id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let org_oid = parse_oid(&org_id)?;
    let site_oid = parse_oid(&site_id)?;
    ensure_member(&state, org_oid, auth.user_id).await?;
    ensure_site_in_org(&state, org_oid, site_oid).await?;
    tracing::info!(org_id = %org_oid, site_id = %site_oid, user_id = %auth.user_id, "realtime query");

    let bytes = site_oid.bytes();
    let ch_site_id = u64::from_be_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ]);

    let stats = state
        .realtime
        .get_current_visitors(ch_site_id)
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

    // Validate all sites belong to org
    let org_sites = state.sites.find_by_org(org_oid).await?;
    let org_site_ids: std::collections::HashSet<String> = org_sites
        .iter()
        .filter_map(|s| s.id.map(|id| id.to_hex()))
        .collect();

    let mut ch_site_ids = Vec::new();
    for sid in &site_id_strs {
        if !org_site_ids.contains(*sid) {
            return Err(ApiError::BadRequest(format!("Site {sid} not in org")));
        }
        let site_oid = parse_oid(sid)?;
        let bytes = site_oid.bytes();
        ch_site_ids.push(u64::from_be_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]));
    }

    tracing::info!(org_id = %org_oid, sites = ch_site_ids.len(), user_id = %auth.user_id, "multi-site realtime query");

    let stats = state
        .realtime
        .get_current_visitors_multi(&ch_site_ids)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(serde_json::to_value(stats).unwrap_or_default()))
}
