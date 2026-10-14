//! Funnels of a site (PS4): the definitions, under the same rules as goals
//! (members read, admins and owners write), and their report.

use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use bson::oid::ObjectId;
use purestat_db::models::{Funnel, FunnelStepType};
use purestat_services::analytics::funnel::{
    BreakdownRow, FunnelBody, FunnelDefinition, FunnelQuery, FunnelReportRequest, StepCondition,
    StepNumbers, goal_missing, step_label,
};
use purestat_services::analytics::query::QueryError;
use purestat_services::analytics::scope::SiteScope;
use serde::Serialize;

use crate::error::ApiError;
use crate::extractors::auth::AuthUser;
use crate::routes::org::{ensure_admin, ensure_member, ensure_site_in_org, parse_oid};
use crate::state::AppState;

#[derive(Serialize)]
pub struct FunnelResponse {
    pub id: String,
    pub site_id: String,
    pub name: String,
    pub steps: Vec<StepResponse>,
}

#[derive(Serialize)]
pub struct StepResponse {
    #[serde(rename = "type")]
    pub step_type: FunnelStepType,
    pub pattern: Option<String>,
    pub goal_id: Option<String>,
    pub label: Option<String>,
}

fn funnel_to_response(funnel: &Funnel) -> FunnelResponse {
    FunnelResponse {
        id: funnel.id.map(|id| id.to_hex()).unwrap_or_default(),
        site_id: funnel.site_id.to_hex(),
        name: funnel.name.clone(),
        steps: funnel
            .steps
            .iter()
            .map(|s| StepResponse {
                step_type: s.step_type,
                pattern: s.pattern.clone(),
                goal_id: s.goal_id.map(|id| id.to_hex()),
                label: s.label.clone(),
            })
            .collect(),
    }
}

fn not_found() -> ApiError {
    ApiError::NotFound("Funnel not found".to_string())
}

/// The body, or a 400 that says what is wrong with it.
fn parse_body<T: serde::de::DeserializeOwned>(body: &Bytes) -> Result<T, ApiError> {
    serde_json::from_slice(body)
        .map_err(|e| ApiError::BadRequest(format!("invalid request body: {e}")))
}

/// A checked definition for the site, or a 422 with the reason.
async fn definition(
    state: &AppState,
    site_oid: ObjectId,
    body: &Bytes,
) -> Result<FunnelDefinition, ApiError> {
    let body: FunnelBody = parse_body(body)?;
    let goals = state.goals.find_by_site(site_oid).await?;
    FunnelDefinition::check(body, &goals).map_err(ApiError::Validation)
}

pub async fn list(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((org_id, site_id)): Path<(String, String)>,
) -> Result<Json<Vec<FunnelResponse>>, ApiError> {
    let org_oid = parse_oid(&org_id)?;
    let site_oid = parse_oid(&site_id)?;
    ensure_member(&state, org_oid, auth.user_id).await?;
    ensure_site_in_org(&state, org_oid, site_oid).await?;

    let funnels = state.funnels.find_by_site(site_oid).await?;
    Ok(Json(funnels.iter().map(funnel_to_response).collect()))
}

pub async fn get(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((org_id, site_id, funnel_id)): Path<(String, String, String)>,
) -> Result<Json<FunnelResponse>, ApiError> {
    let org_oid = parse_oid(&org_id)?;
    let site_oid = parse_oid(&site_id)?;
    let funnel_oid = parse_oid(&funnel_id)?;
    ensure_member(&state, org_oid, auth.user_id).await?;
    ensure_site_in_org(&state, org_oid, site_oid).await?;

    let funnel = state
        .funnels
        .find(funnel_oid, site_oid)
        .await?
        .ok_or_else(not_found)?;
    Ok(Json(funnel_to_response(&funnel)))
}

pub async fn create(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((org_id, site_id)): Path<(String, String)>,
    body: Bytes,
) -> Result<(StatusCode, Json<FunnelResponse>), ApiError> {
    let org_oid = parse_oid(&org_id)?;
    let site_oid = parse_oid(&site_id)?;
    ensure_admin(&state, org_oid, auth.user_id).await?;
    ensure_site_in_org(&state, org_oid, site_oid).await?;

    let def = definition(&state, site_oid, &body).await?;
    let funnel = state
        .funnels
        .create(site_oid, org_oid, def.name, def.steps)
        .await?;
    Ok((StatusCode::CREATED, Json(funnel_to_response(&funnel))))
}

pub async fn update(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((org_id, site_id, funnel_id)): Path<(String, String, String)>,
    body: Bytes,
) -> Result<Json<FunnelResponse>, ApiError> {
    let org_oid = parse_oid(&org_id)?;
    let site_oid = parse_oid(&site_id)?;
    let funnel_oid = parse_oid(&funnel_id)?;
    ensure_admin(&state, org_oid, auth.user_id).await?;
    ensure_site_in_org(&state, org_oid, site_oid).await?;

    let def = definition(&state, site_oid, &body).await?;
    let funnel = state
        .funnels
        .update(funnel_oid, site_oid, def.name, def.steps)
        .await?
        .ok_or_else(not_found)?;
    Ok(Json(funnel_to_response(&funnel)))
}

pub async fn delete(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((org_id, site_id, funnel_id)): Path<(String, String, String)>,
) -> Result<StatusCode, ApiError> {
    let org_oid = parse_oid(&org_id)?;
    let site_oid = parse_oid(&site_id)?;
    let funnel_oid = parse_oid(&funnel_id)?;
    ensure_admin(&state, org_oid, auth.user_id).await?;
    ensure_site_in_org(&state, org_oid, site_oid).await?;

    if state.funnels.delete(funnel_oid, site_oid).await? == 0 {
        return Err(not_found());
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize)]
pub struct StepReport {
    pub label: String,
    #[serde(rename = "type")]
    pub step_type: FunnelStepType,
    /// A goal step whose goal was deleted: nothing completes it.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub goal_missing: bool,
    #[serde(flatten)]
    pub numbers: StepNumbers,
}

#[derive(Serialize)]
pub struct BreakdownReport {
    pub dimension: String,
    pub rows: Vec<BreakdownRow>,
    pub has_more: bool,
}

#[derive(Serialize)]
pub struct ReportMeta {
    pub date_from: String,
    pub date_to: String,
    pub hostnames: Vec<String>,
}

#[derive(Serialize)]
pub struct FunnelReportResponse {
    pub funnel: FunnelResponse,
    pub steps: Vec<StepReport>,
    pub breakdown: Option<BreakdownReport>,
    pub meta: ReportMeta,
}

/// `POST .../funnel/{funnel_id}/stats`: the funnel's numbers for a range of
/// days. In the stats rate-limit group, like every path ending in `/stats`.
pub async fn stats(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((org_id, site_id, funnel_id)): Path<(String, String, String)>,
    body: Bytes,
) -> Result<Json<FunnelReportResponse>, ApiError> {
    let org_oid = parse_oid(&org_id)?;
    let site_oid = parse_oid(&site_id)?;
    let funnel_oid = parse_oid(&funnel_id)?;
    ensure_member(&state, org_oid, auth.user_id).await?;
    let site = ensure_site_in_org(&state, org_oid, site_oid).await?;
    let funnel = state
        .funnels
        .find(funnel_oid, site_oid)
        .await?
        .ok_or_else(not_found)?;
    let request: FunnelReportRequest = parse_body(&body)?;
    let query = FunnelQuery::parse(request).map_err(ApiError::BadRequest)?;
    let goals = state.goals.find_by_site(site_oid).await?;
    let conditions: Vec<StepCondition> = funnel
        .steps
        .iter()
        .map(|s| StepCondition::of(s, &goals))
        .collect();
    let scope = SiteScope::for_site(&site);
    tracing::info!(org_id = %org_oid, site_id = %site_oid, funnel_id = %funnel_oid, user_id = %auth.user_id, "funnel query");

    let report = state
        .query
        .query_funnel(&scope, &conditions, &query)
        .await
        .map_err(|e| match e {
            QueryError::InvalidQuery(msg) => ApiError::BadRequest(msg),
            e => ApiError::Internal(e.to_string()),
        })?;

    let steps = funnel
        .steps
        .iter()
        .zip(report.steps)
        .map(|(step, numbers)| StepReport {
            label: step_label(step, &goals),
            step_type: step.step_type,
            goal_missing: goal_missing(step, &goals),
            numbers,
        })
        .collect();
    let breakdown = query
        .breakdown
        .zip(report.breakdown)
        .map(|(dimension, rows)| BreakdownReport {
            dimension: dimension.name().to_string(),
            rows,
            has_more: report.has_more,
        });
    Ok(Json(FunnelReportResponse {
        funnel: funnel_to_response(&funnel),
        steps,
        breakdown,
        meta: ReportMeta {
            date_from: query.dates.from_str(),
            date_to: query.dates.to_str(),
            hostnames: scope.hostnames,
        },
    }))
}
