//! `POST /api/v1/query`: the stats of the site an API key belongs to, as
//! `docs/api.md` describes. Its errors are `{"error": "..."}`, a shape of
//! their own, because they are part of that contract.

use axum::Json;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use purestat_services::analytics::query::QueryError;
use purestat_services::analytics::scope::SiteScope;
use purestat_services::analytics::v1::{V1Query, V1Request, V1Row};
use serde::Serialize;

use crate::extractors::api_key::ApiKeyAuth;
use crate::state::AppState;

#[derive(Debug)]
pub enum V1Error {
    /// 401 `{"error":"unauthorized"}`: no key, an unknown or revoked one, or
    /// one without `stats:read`.
    Unauthorized,
    /// 400 `{"error": <what is wrong>}`.
    BadRequest(String),
    /// 500 `{"error":"internal"}`; the cause is logged, not returned.
    Internal(String),
}

impl IntoResponse for V1Error {
    fn into_response(self) -> Response {
        let (status, error) = match self {
            V1Error::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized".to_string()),
            V1Error::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            V1Error::Internal(msg) => {
                tracing::error!(error = %msg, "v1 query failed");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal".to_string())
            }
        };
        (status, Json(serde_json::json!({ "error": error }))).into_response()
    }
}

#[derive(Serialize)]
pub struct V1Response {
    pub rows: Vec<V1Row>,
    pub meta: V1Meta,
}

#[derive(Serialize)]
pub struct V1Meta {
    pub site_domain: String,
    pub hostnames: Vec<String>,
    pub date_from: String,
    pub date_to: String,
    pub limit: u64,
    pub offset: u64,
    pub has_more: bool,
}

pub async fn query(
    State(state): State<AppState>,
    auth: ApiKeyAuth,
    body: Bytes,
) -> Result<Json<V1Response>, V1Error> {
    let request: V1Request = serde_json::from_slice(&body)
        .map_err(|e| V1Error::BadRequest(format!("invalid request body: {e}")))?;
    let site_oid = auth
        .site
        .id
        .ok_or_else(|| V1Error::Internal("a stored site without an id".to_string()))?;
    let goals = state
        .goals
        .find_by_site(site_oid)
        .await
        .map_err(|e| V1Error::Internal(e.to_string()))?;
    let query = V1Query::parse(request, &goals).map_err(V1Error::BadRequest)?;
    let scope = SiteScope::for_site(&auth.site);
    tracing::info!(site_id = %site_oid, key_id = %auth.key_id, "v1 query");

    let result = state
        .query
        .query_v1(&scope, &query)
        .await
        .map_err(|e| match e {
            QueryError::InvalidQuery(msg) => V1Error::BadRequest(msg),
            e => V1Error::Internal(e.to_string()),
        })?;

    Ok(Json(V1Response {
        rows: result.rows,
        meta: V1Meta {
            site_domain: auth.site.domain,
            hostnames: scope.hostnames,
            date_from: query.dates.from_str(),
            date_to: query.dates.to_str(),
            limit: query.limit,
            offset: query.offset,
            has_more: result.has_more,
        },
    }))
}
