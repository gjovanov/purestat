use axum::extract::{Path, Query, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use crate::error::ApiError;
use crate::extractors::auth::AuthUser;
use crate::routes::org::{ensure_member, parse_oid};
use crate::state::AppState;

#[derive(Deserialize)]
pub struct ExportQuery {
    pub date_from: String,
    pub date_to: String,
}

pub async fn export_csv(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((org_id, site_id)): Path<(String, String)>,
    Query(query): Query<ExportQuery>,
) -> Result<Response, ApiError> {
    let org_oid = parse_oid(&org_id)?;
    let site_oid = parse_oid(&site_id)?;
    ensure_member(&state, org_oid, auth.user_id).await?;

    let bytes = site_oid.bytes();
    let ch_site_id = u64::from_be_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ]);

    let csv = state
        .export
        .export_csv(ch_site_id, &query.date_from, &query.date_to)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let response = (
        [
            (header::CONTENT_TYPE, "text/csv"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"purestat-export.csv\"",
            ),
        ],
        csv,
    )
        .into_response();

    Ok(response)
}
