//! `POST /api/org/{org_id}/site/{site_id}/journey/stats` (PS5): the paths
//! visitors took from an entry page, or into a goal, as nodes and links for a
//! Sankey. Every path shown was followed by at least 5 visitors; the query
//! itself drops the rest (`analytics::journey`).

use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path, State};
use purestat_services::analytics::journey::{
    Anchor, JourneyLink, JourneyNode, JourneyQuery, JourneyRequest, MIN_VISITORS,
};
use purestat_services::analytics::query::QueryError;
use purestat_services::analytics::scope::SiteScope;
use serde::Serialize;

use crate::error::ApiError;
use crate::extractors::auth::AuthUser;
use crate::routes::org::{ensure_member, ensure_site_in_org, parse_oid};
use crate::state::AppState;

#[derive(Serialize)]
pub struct AnchorResponse {
    /// `entry_page` or `goal`.
    #[serde(rename = "type")]
    pub anchor_type: &'static str,
    /// The page, or the goal's name.
    pub label: String,
}

#[derive(Serialize)]
pub struct JourneyMeta {
    pub date_from: String,
    pub date_to: String,
    pub depth: u8,
    pub width: usize,
    /// The fewest distinct visitors a shown path has.
    pub min_visitors: u64,
    pub hostnames: Vec<String>,
}

#[derive(Serialize)]
pub struct JourneyResponse {
    pub anchor: AnchorResponse,
    pub nodes: Vec<JourneyNode>,
    pub links: Vec<JourneyLink>,
    pub meta: JourneyMeta,
}

pub async fn stats(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((org_id, site_id)): Path<(String, String)>,
    body: Bytes,
) -> Result<Json<JourneyResponse>, ApiError> {
    let org_oid = parse_oid(&org_id)?;
    let site_oid = parse_oid(&site_id)?;
    ensure_member(&state, org_oid, auth.user_id).await?;
    let site = ensure_site_in_org(&state, org_oid, site_oid).await?;
    let request: JourneyRequest = serde_json::from_slice(&body)
        .map_err(|e| ApiError::BadRequest(format!("invalid request body: {e}")))?;
    let goals = state.goals.find_by_site(site_oid).await?;
    let query = JourneyQuery::parse(request, &goals).map_err(ApiError::BadRequest)?;
    let scope = SiteScope::for_site(&site);
    tracing::info!(org_id = %org_oid, site_id = %site_oid, user_id = %auth.user_id, "journey query");

    let journey = state
        .query
        .query_journey(&scope, &query)
        .await
        .map_err(|e| match e {
            QueryError::InvalidQuery(msg) => ApiError::BadRequest(msg),
            e => ApiError::Internal(e.to_string()),
        })?;

    let anchor = match &query.anchor {
        Anchor::EntryPage(page) => AnchorResponse {
            anchor_type: "entry_page",
            label: page.clone(),
        },
        Anchor::Goal(goal) => AnchorResponse {
            anchor_type: "goal",
            label: goal.name.clone(),
        },
    };
    Ok(Json(JourneyResponse {
        anchor,
        nodes: journey.nodes,
        links: journey.links,
        meta: JourneyMeta {
            date_from: query.dates.from_str(),
            date_to: query.dates.to_str(),
            depth: query.depth,
            width: query.width,
            min_visitors: MIN_VISITORS,
            hostnames: scope.hostnames,
        },
    }))
}
