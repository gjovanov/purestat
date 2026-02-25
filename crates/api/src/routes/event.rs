use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use purestat_db::clickhouse::schemas::Event;
use serde::Deserialize;
use std::collections::HashMap;

use crate::error::ApiError;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct EventRequest {
    pub domain: String,
    pub name: String,
    pub url: String,
    pub referrer: Option<String>,
    pub screen_width: Option<u16>,
    pub props: Option<HashMap<String, String>>,
}

pub async fn ingest(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<EventRequest>,
) -> Result<StatusCode, ApiError> {
    // Resolve site by domain
    let site = state
        .sites
        .find_by_domain(&body.domain)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .ok_or_else(|| ApiError::BadRequest(format!("Unknown domain: {}", body.domain)))?;

    let site_id = site
        .id
        .map(|id| {
            // Convert ObjectId to u64 by taking first 8 bytes
            let bytes = id.bytes();
            u64::from_be_bytes([
                bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
            ])
        })
        .unwrap_or(0);

    // Extract IP and User-Agent for privacy hashing
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

    // Generate privacy-preserving visitor hash
    let visitor_hash = state
        .privacy
        .generate_visitor_hash(&body.domain, &ip, &user_agent)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    // Parse URL for path and hostname
    let (path, hostname) = parse_url(&body.url);

    // Parse referrer source
    let referrer = body.referrer.clone().unwrap_or_default();
    let referrer_source = parse_referrer_source(&referrer);

    // Parse props
    let (prop_keys, prop_values) = match &body.props {
        Some(props) => {
            let keys: Vec<String> = props.keys().cloned().collect();
            let values: Vec<String> = props.values().cloned().collect();
            (keys, values)
        }
        None => (vec![], vec![]),
    };

    // Parse device info from user agent (simplified)
    let (browser, os, device_type) = parse_user_agent(&user_agent);

    let session_id = format!("{}-{}", visitor_hash.get(..16).unwrap_or(""), "session");
    let event_name = body.name.clone();
    let is_pageview = body.name == "pageview";

    let event = Event {
        site_id,
        visitor_hash,
        session_id,
        event_name,
        url: body.url,
        path,
        hostname,
        referrer,
        referrer_source,
        utm_source: String::new(),
        utm_medium: String::new(),
        utm_campaign: String::new(),
        utm_content: String::new(),
        utm_term: String::new(),
        country: String::new(),
        region: String::new(),
        city: String::new(),
        browser,
        browser_version: String::new(),
        os,
        os_version: String::new(),
        device_type,
        screen_width: body.screen_width.unwrap_or(0),
        screen_height: 0,
        prop_keys,
        prop_values,
        revenue_amount: None,
        revenue_currency: None,
        timestamp: time::OffsetDateTime::now_utc(),
    };

    state
        .ingest
        .ingest(event)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    // Increment org pageview counter
    if is_pageview {
        let _ = state.orgs.increment_pageviews(site.org_id, 1).await;
    }

    Ok(StatusCode::ACCEPTED)
}

fn parse_url(url: &str) -> (String, String) {
    if let Ok(parsed) = url::Url::parse(url) {
        (
            parsed.path().to_string(),
            parsed.host_str().unwrap_or("").to_string(),
        )
    } else {
        (url.to_string(), String::new())
    }
}

fn parse_referrer_source(referrer: &str) -> String {
    if referrer.is_empty() {
        return "Direct".to_string();
    }
    if let Ok(parsed) = url::Url::parse(referrer) {
        let host = parsed.host_str().unwrap_or("");
        if host.contains("google") {
            "Google".to_string()
        } else if host.contains("bing") {
            "Bing".to_string()
        } else if host.contains("twitter") || host.contains("x.com") {
            "Twitter".to_string()
        } else if host.contains("facebook") {
            "Facebook".to_string()
        } else if host.contains("linkedin") {
            "LinkedIn".to_string()
        } else if host.contains("reddit") {
            "Reddit".to_string()
        } else if host.contains("github") {
            "GitHub".to_string()
        } else {
            host.to_string()
        }
    } else {
        referrer.to_string()
    }
}

fn parse_user_agent(ua: &str) -> (String, String, String) {
    let ua_lower = ua.to_lowercase();

    let browser = if ua_lower.contains("firefox") {
        "Firefox"
    } else if ua_lower.contains("edg/") {
        "Edge"
    } else if ua_lower.contains("chrome") {
        "Chrome"
    } else if ua_lower.contains("safari") {
        "Safari"
    } else {
        "Other"
    }
    .to_string();

    let os = if ua_lower.contains("windows") {
        "Windows"
    } else if ua_lower.contains("mac os") || ua_lower.contains("macos") {
        "macOS"
    } else if ua_lower.contains("linux") {
        "Linux"
    } else if ua_lower.contains("android") {
        "Android"
    } else if ua_lower.contains("iphone") || ua_lower.contains("ipad") {
        "iOS"
    } else {
        "Other"
    }
    .to_string();

    let device_type = if ua_lower.contains("mobile") || ua_lower.contains("android") {
        "mobile"
    } else if ua_lower.contains("tablet") || ua_lower.contains("ipad") {
        "tablet"
    } else {
        "desktop"
    }
    .to_string();

    (browser, os, device_type)
}
