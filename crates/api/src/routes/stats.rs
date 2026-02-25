use axum::extract::{Path, State};
use axum::Json;
use purestat_services::analytics::query::StatsQuery;

use crate::error::ApiError;
use crate::extractors::auth::AuthUser;
use crate::routes::org::{ensure_member, parse_oid};
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

    // Convert ObjectId to u64 for ClickHouse
    let bytes = site_oid.bytes();
    let ch_site_id = u64::from_be_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ]);

    let result = state
        .query
        .query_stats(ch_site_id, &body)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(serde_json::to_value(result).unwrap_or_default()))
}
