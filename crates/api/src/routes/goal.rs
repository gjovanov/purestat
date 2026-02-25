use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use purestat_db::models::GoalType;
use serde::{Deserialize, Serialize};

use crate::error::ApiError;
use crate::extractors::auth::AuthUser;
use crate::routes::org::{ensure_admin, ensure_member, parse_oid};
use crate::state::AppState;

#[derive(Deserialize)]
pub struct CreateGoalRequest {
    pub goal_type: GoalType,
    pub name: String,
    pub event_name: Option<String>,
    pub page_path: Option<String>,
}

#[derive(Serialize)]
pub struct GoalResponse {
    pub id: String,
    pub site_id: String,
    pub goal_type: String,
    pub name: String,
    pub event_name: Option<String>,
    pub page_path: Option<String>,
}

pub async fn list(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((org_id, site_id)): Path<(String, String)>,
) -> Result<Json<Vec<GoalResponse>>, ApiError> {
    let org_oid = parse_oid(&org_id)?;
    let site_oid = parse_oid(&site_id)?;
    ensure_member(&state, org_oid, auth.user_id).await?;

    let goals = state.goals.find_by_site(site_oid).await?;
    Ok(Json(goals.iter().map(goal_to_response).collect()))
}

pub async fn create(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((org_id, site_id)): Path<(String, String)>,
    Json(body): Json<CreateGoalRequest>,
) -> Result<(StatusCode, Json<GoalResponse>), ApiError> {
    let org_oid = parse_oid(&org_id)?;
    let site_oid = parse_oid(&site_id)?;
    ensure_admin(&state, org_oid, auth.user_id).await?;

    let goal = state
        .goals
        .create(
            site_oid,
            org_oid,
            body.goal_type,
            body.name,
            body.event_name,
            body.page_path,
        )
        .await?;

    Ok((StatusCode::CREATED, Json(goal_to_response(&goal))))
}

pub async fn delete(
    State(state): State<AppState>,
    auth: AuthUser,
    Path((org_id, _site_id, goal_id)): Path<(String, String, String)>,
) -> Result<StatusCode, ApiError> {
    let org_oid = parse_oid(&org_id)?;
    let goal_oid = parse_oid(&goal_id)?;
    ensure_admin(&state, org_oid, auth.user_id).await?;

    state.goals.delete(goal_oid).await?;
    Ok(StatusCode::NO_CONTENT)
}

fn goal_to_response(goal: &purestat_db::models::Goal) -> GoalResponse {
    GoalResponse {
        id: goal.id.map(|id| id.to_hex()).unwrap_or_default(),
        site_id: goal.site_id.to_hex(),
        goal_type: serde_json::to_string(&goal.goal_type)
            .unwrap_or_default()
            .trim_matches('"')
            .to_string(),
        name: goal.name.clone(),
        event_name: goal.event_name.clone(),
        page_path: goal.page_path.clone(),
    }
}
