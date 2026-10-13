use axum::extract::{Path, Query, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use purestat_services::analytics::scope::SiteScope;
use purestat_services::export::ExportError;

use crate::error::ApiError;
use crate::extractors::auth::AuthUser;
use crate::routes::org::{ensure_member, ensure_site_in_org, parse_oid};
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
    let site = ensure_site_in_org(&state, org_oid, site_oid).await?;
    tracing::info!(org_id = %org_oid, site_id = %site_oid, user_id = %auth.user_id, "csv export");

    let csv = state
        .export
        .export_csv(&SiteScope::for_site(&site), &query.date_from, &query.date_to)
        .await
        .map_err(|e| match e {
            ExportError::InvalidDate(d) => ApiError::BadRequest(format!("Invalid date: {d}")),
            e => ApiError::Internal(e.to_string()),
        })?;

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
