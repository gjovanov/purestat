//! The dashboard's reports: the v1 query (`docs/api.md`, `POST /api/v1/query`)
//! for a signed-in member, on one site or on several sites of the org at
//! once. Same body, same rows, same engine as an API key gets, so the
//! numbers on screen are the numbers the API reports.

use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use purestat_services::analytics::query::QueryError;
use purestat_services::analytics::scope::SiteScope;
use purestat_services::analytics::v1::{V1Query, V1Request, V1Row};
use serde::Serialize;

use crate::error::ApiError;
use crate::extractors::auth::AuthUser;
use crate::routes::org::{ensure_member, ensure_site_in_org, parse_oid};
use crate::routes::realtime::MultiSiteQuery;
use crate::routes::stats::org_site_scopes;
use crate::routes::v1::{V1Meta, V1Response};
use crate::state::AppState;

fn parse_body(body: &Bytes) -> Result<V1Request, ApiError> {
    serde_json::from_slice(body)
        .map_err(|e| ApiError::BadRequest(format!("invalid request body: {e}")))
}

fn query_error(e: QueryError) -> ApiError {
    match e {
        QueryError::InvalidQuery(msg) => ApiError::BadRequest(msg),
        e => ApiError::Internal(e.to_string()),
    }
}

/// `POST /api/org/{org_id}/site/{site_id}/query`: one site, with its goals.
pub async fn site_query(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((org_id, site_id)): Path<(String, String)>,
    body: Bytes,
) -> Result<Json<V1Response>, ApiError> {
    let org_oid = parse_oid(&org_id)?;
    let site_oid = parse_oid(&site_id)?;
    ensure_member(&state, org_oid, auth.user_id).await?;
    let site = ensure_site_in_org(&state, org_oid, site_oid).await?;
    let request = parse_body(&body)?;
    let goals = state.goals.find_by_site(site_oid).await?;
    let query = V1Query::parse(request, &goals).map_err(ApiError::BadRequest)?;
    let scope = SiteScope::for_site(&site);
    tracing::info!(org_id = %org_oid, site_id = %site_oid, user_id = %auth.user_id, "report query");

    let result = state
        .query
        .query_v1(&scope, &query)
        .await
        .map_err(query_error)?;

    Ok(Json(V1Response {
        rows: result.rows,
        meta: V1Meta {
            site_domain: site.domain,
            hostnames: scope.hostnames,
            date_from: query.dates.from_str(),
            date_to: query.dates.to_str(),
            limit: query.limit,
            offset: query.offset,
            has_more: result.has_more,
        },
    }))
}

#[derive(Serialize)]
pub struct OrgReportResponse {
    pub rows: Vec<V1Row>,
    pub meta: OrgReportMeta,
}

#[derive(Serialize)]
pub struct OrgReportMeta {
    pub sites: usize,
    pub date_from: String,
    pub date_to: String,
    pub limit: u64,
    pub offset: u64,
    pub has_more: bool,
}

/// `POST /api/org/{org_id}/analytics/query?site_ids=a,b`: several sites of
/// the org as one. Goals belong to a site, so this query names none.
pub async fn org_query(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(org_id): Path<String>,
    Query(sites): Query<MultiSiteQuery>,
    body: Bytes,
) -> Result<Json<OrgReportResponse>, ApiError> {
    let org_oid = parse_oid(&org_id)?;
    ensure_member(&state, org_oid, auth.user_id).await?;
    let site_ids: Vec<&str> = sites
        .site_ids
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if site_ids.is_empty() {
        return Err(ApiError::BadRequest("No site IDs provided".to_string()));
    }
    let scopes = org_site_scopes(&state, org_oid, site_ids).await?;
    let request = parse_body(&body)?;
    let query = V1Query::parse(request, &[]).map_err(ApiError::BadRequest)?;
    tracing::info!(org_id = %org_oid, sites = scopes.len(), user_id = %auth.user_id, "multi-site report query");

    let result = state
        .query
        .query_v1_sites(&scopes, &query)
        .await
        .map_err(query_error)?;

    Ok(Json(OrgReportResponse {
        rows: result.rows,
        meta: OrgReportMeta {
            sites: scopes.len(),
            date_from: query.dates.from_str(),
            date_to: query.dates.to_str(),
            limit: query.limit,
            offset: query.offset,
            has_more: result.has_more,
        },
    }))
}
