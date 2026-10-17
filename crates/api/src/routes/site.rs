use std::time::Duration;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use purestat_services::analytics::purge::SiteRows;
use serde::{Deserialize, Serialize};

use crate::error::ApiError;
use crate::extractors::auth::AuthUser;
use crate::routes::org::{ensure_admin, ensure_member, ensure_site_in_org, parse_oid};
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
    ensure_site_in_org(&state, org_oid, site_oid).await?;

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

/// Deletes the site, its goals and funnels, and its rows in ClickHouse
/// (`purge`). The rows go first: a ClickHouse that refuses the deletion
/// leaves the site as it was, rather than leaving rows no site maps to any
/// more. ClickHouse removes them in the background; the answer comes once
/// the deletion is submitted.
pub async fn delete(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((org_id, site_id)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    let org_oid = parse_oid(&org_id)?;
    let site_oid = parse_oid(&site_id)?;
    ensure_admin(&state, org_oid, auth.user_id).await?;
    let site = ensure_site_in_org(&state, org_oid, site_oid).await?;

    let sharers = state.sites.sharing_clickhouse_id(&site).await?;
    let rows = SiteRows::of(&site, &sharers);
    if rows.is_empty() {
        tracing::warn!(
            site_id = rows.site_id,
            "The site shares its ClickHouse id and has no hostname of its own: no rows to delete"
        );
    }
    if let Err(e) = state.purge.submit(&rows).await {
        tracing::error!(error = %e, site_id = rows.site_id, "Could not delete the site's rows in ClickHouse; the site is kept");
        return Err(ApiError::Unavailable(
            "The site's analytics could not be deleted, so the site was kept. Try again in a minute."
                .to_string(),
        ));
    }

    state.goals.delete_all_for_site(site_oid).await?;
    state.funnels.delete_all_for_site(site_oid).await?;
    state.sites.delete(site_oid).await?;

    delete_rows_in_flight(&state, &rows).await;
    Ok(StatusCode::NO_CONTENT)
}

/// How long the deletion waits for an event flush in flight.
const FLUSH_WAIT: Duration = Duration::from_secs(10);

/// The rows that were on their way in while the site existed: its buffered
/// events, and its live visits, which the sweeper writes 30 minutes after
/// their last event. The site's Mongo row is gone, so ingest takes no more of
/// its events; this drops the ones it holds, then deletes again for whatever
/// was written since the first deletion. An event already past its site
/// lookup when the row went can still slip through: that window is one
/// request long. The buffer and the visits are this process's, and
/// production runs one API process. Failures are logged; the site is
/// deleted either way.
async fn delete_rows_in_flight(state: &AppState, rows: &SiteRows) {
    if rows.is_empty() {
        return;
    }
    if let Err(e) = state
        .ingest
        .discard(|event| rows.owns_event(event), FLUSH_WAIT)
        .await
    {
        tracing::warn!(error = %e, site_id = rows.site_id, "Could not drop a deleted site's buffered events");
    }
    // A visit records no hostname, so only a site that owns its id can tell
    // its live visits apart.
    if rows.owns_the_id()
        && let Err(e) = state.session.discard_site(rows.site_id).await
    {
        tracing::warn!(error = %e, site_id = rows.site_id, "Could not drop a deleted site's live visits");
    }
    if let Err(e) = state.purge.submit(rows).await {
        tracing::error!(
            error = %e,
            site_id = rows.site_id,
            "Could not repeat a deleted site's ClickHouse deletion: rows written while it was deleted may remain"
        );
    }
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
