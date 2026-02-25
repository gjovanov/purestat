use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use purestat_db::clickhouse::schemas::Event;
use purestat_services::analytics::ingest::IngestService;
use purestat_services::analytics::privacy::PrivacyEngine;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

#[derive(Clone)]
pub struct TrackerState {
    pub ingest: Arc<IngestService>,
    pub privacy: Arc<PrivacyEngine>,
}

#[derive(Deserialize)]
pub struct EventRequest {
    pub domain: String,
    pub name: String,
    pub url: String,
    pub referrer: Option<String>,
    pub screen_width: Option<u16>,
    pub props: Option<HashMap<String, String>>,
}

#[derive(Serialize)]
struct HealthResponse {
    status: String,
}

pub fn build_tracker_router(state: TrackerState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/api/event", post(ingest_event))
        .route("/health", get(health))
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state)
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
    })
}

async fn ingest_event(
    State(state): State<TrackerState>,
    headers: HeaderMap,
    Json(body): Json<EventRequest>,
) -> StatusCode {
    let ip = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .unwrap_or("0.0.0.0")
        .trim()
        .to_string();

    let user_agent = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    let visitor_hash = match state
        .privacy
        .generate_visitor_hash(&body.domain, &ip, &user_agent)
        .await
    {
        Ok(h) => h,
        Err(e) => {
            tracing::error!(error = %e, "Failed to generate visitor hash");
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };

    let (path, hostname) = if let Ok(parsed) = url::Url::parse(&body.url) {
        (
            parsed.path().to_string(),
            parsed.host_str().unwrap_or("").to_string(),
        )
    } else {
        (body.url.clone(), String::new())
    };

    let (prop_keys, prop_values) = match &body.props {
        Some(props) => (
            props.keys().cloned().collect(),
            props.values().cloned().collect(),
        ),
        None => (vec![], vec![]),
    };

    let session_id = format!("{}-session", visitor_hash.get(..16).unwrap_or(""));

    let event = Event {
        site_id: 0, // Will be resolved by domain lookup in full API
        visitor_hash,
        session_id,
        event_name: body.name,
        url: body.url,
        path,
        hostname,
        referrer: body.referrer.unwrap_or_default(),
        referrer_source: String::new(),
        utm_source: String::new(),
        utm_medium: String::new(),
        utm_campaign: String::new(),
        utm_content: String::new(),
        utm_term: String::new(),
        country: String::new(),
        region: String::new(),
        city: String::new(),
        browser: String::new(),
        browser_version: String::new(),
        os: String::new(),
        os_version: String::new(),
        device_type: String::new(),
        screen_width: body.screen_width.unwrap_or(0),
        screen_height: 0,
        prop_keys,
        prop_values,
        revenue_amount: None,
        revenue_currency: None,
        timestamp: time::OffsetDateTime::now_utc(),
    };

    match state.ingest.ingest(event).await {
        Ok(_) => StatusCode::ACCEPTED,
        Err(e) => {
            tracing::error!(error = %e, "Failed to ingest event");
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}
