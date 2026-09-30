use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
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
    raw_body: Bytes,
) -> Result<StatusCode, ApiError> {
    let body: EventRequest = serde_json::from_slice(&raw_body)
        .map_err(|e| ApiError::BadRequest(format!("Invalid event body: {e}")))?;
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
    // Priority: X-Forwarded-For (first entry) > X-Real-IP > fallback
    let ip = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.trim())
        })
        .unwrap_or("0.0.0.0")
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

    // Parse the page address once: what is stored, its path and host, and the
    // campaign parameters.
    let page = parse_page_url(&body.url);

    // The referrer is kept as far as its path; its source is read from the host.
    let referrer = clean_referrer(body.referrer.as_deref().unwrap_or(""));
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

    // GeoIP lookup
    let geo = state.geo.lookup(&ip);

    // Session tracking
    let session_data = purestat_services::analytics::session::EventSessionData {
        site_id,
        visitor_hash: visitor_hash.clone(),
        event_name: body.name.clone(),
        path: page.path.clone(),
        referrer: referrer.clone(),
        referrer_source: referrer_source.clone(),
        utm_source: page.utm_source.clone(),
        utm_medium: page.utm_medium.clone(),
        utm_campaign: page.utm_campaign.clone(),
        utm_content: page.utm_content.clone(),
        utm_term: page.utm_term.clone(),
        country: geo.country.clone(),
        browser: browser.clone(),
        os: os.clone(),
        device_type: device_type.clone(),
    };
    let session_id = state
        .session
        .track_event(&session_data)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!(error = %e, "Session tracking failed, using fallback");
            format!("{}-{}", visitor_hash.get(..16).unwrap_or(""), "fallback")
        });

    let event_name = body.name.clone();
    let is_pageview = body.name == "pageview";

    let event = Event {
        site_id,
        visitor_hash,
        session_id,
        event_name,
        url: page.url,
        path: page.path,
        hostname: page.hostname,
        referrer,
        referrer_source,
        utm_source: page.utm_source,
        utm_medium: page.utm_medium,
        utm_campaign: page.utm_campaign,
        utm_content: page.utm_content,
        utm_term: page.utm_term,
        country: geo.country,
        region: geo.region,
        city: geo.city,
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

/// What ingest keeps of a page address: the address without its query string,
/// fragment or credentials, its path and host, and the five campaign parameters.
///
/// The query and the fragment are dropped because they carry secrets as often
/// as anything else: sign-in callbacks put tokens, OAuth codes and invites
/// there, and a login form submitted with GET puts the password there. No
/// report reads them. The campaign keys are the exception, and they are kept in
/// their own columns, which is where the reports look for them.
#[derive(Debug, Default, PartialEq)]
struct PageUrl {
    url: String,
    path: String,
    hostname: String,
    utm_source: String,
    utm_medium: String,
    utm_campaign: String,
    utm_content: String,
    utm_term: String,
}

/// Longest campaign value kept; anything longer is cut.
const MAX_UTM_CHARS: usize = 256;

fn parse_page_url(raw: &str) -> PageUrl {
    let Ok(mut parsed) = url::Url::parse(raw) else {
        // Not an absolute URL: keep what comes before any query or fragment.
        let bare = strip_query_and_fragment(raw).to_string();
        return PageUrl {
            url: bare.clone(),
            path: bare,
            ..PageUrl::default()
        };
    };
    let mut page = PageUrl {
        path: parsed.path().to_string(),
        hostname: parsed.host_str().unwrap_or("").to_string(),
        ..PageUrl::default()
    };
    for (key, value) in parsed.query_pairs() {
        let slot = match key.as_ref() {
            "utm_source" => &mut page.utm_source,
            "utm_medium" => &mut page.utm_medium,
            "utm_campaign" => &mut page.utm_campaign,
            "utm_content" => &mut page.utm_content,
            "utm_term" => &mut page.utm_term,
            _ => continue,
        };
        // The first occurrence wins, as in every report that reads a campaign.
        if slot.is_empty() {
            *slot = value.chars().take(MAX_UTM_CHARS).collect();
        }
    }
    page.url = without_query_or_credentials(&mut parsed);
    page
}

/// The referrer, kept as far as its path. Its query and fragment are another
/// page's parameters, just as likely to hold a credential as our own.
fn clean_referrer(raw: &str) -> String {
    match url::Url::parse(raw) {
        Ok(mut parsed) => without_query_or_credentials(&mut parsed),
        Err(_) => strip_query_and_fragment(raw).to_string(),
    }
}

fn without_query_or_credentials(parsed: &mut url::Url) -> String {
    parsed.set_query(None);
    parsed.set_fragment(None);
    // Both only fail for URLs that cannot carry credentials in the first place.
    let _ = parsed.set_username("");
    let _ = parsed.set_password(None);
    parsed.to_string()
}

fn strip_query_and_fragment(raw: &str) -> &str {
    raw.split(&['?', '#'][..]).next().unwrap_or("")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_address_keeps_its_campaign_and_nothing_else_from_the_query() {
        let page = parse_page_url(
            "https://example.com/oauth/callback?token=eyJhbGciOi.x.y&utm_source=news%20letter\
             &code=4%2F0Ab&utm_campaign=fall&state=abc#access_token=secret",
        );
        assert_eq!(page.url, "https://example.com/oauth/callback");
        assert_eq!(page.path, "/oauth/callback");
        assert_eq!(page.hostname, "example.com");
        assert_eq!(page.utm_source, "news letter");
        assert_eq!(page.utm_campaign, "fall");
        assert_eq!(page.utm_medium, "");
    }

    #[test]
    fn credentials_in_a_page_address_are_not_kept() {
        let page =
            parse_page_url("https://user:hunter2@example.com/login?username=a&password=hunter2");
        assert_eq!(page.url, "https://example.com/login");
        assert!(!page.url.contains("hunter2"));
    }

    #[test]
    fn an_address_that_does_not_parse_is_cut_at_its_query() {
        let page = parse_page_url("/apps?token=abc#frag");
        assert_eq!(page.url, "/apps");
        assert_eq!(page.path, "/apps");
        assert_eq!(page.hostname, "");
    }

    #[test]
    fn a_repeated_campaign_key_keeps_the_first_and_a_long_one_is_cut() {
        let long = "x".repeat(MAX_UTM_CHARS + 10);
        let page = parse_page_url(&format!(
            "https://example.com/?utm_source=first&utm_source=second&utm_term={long}"
        ));
        assert_eq!(page.utm_source, "first");
        assert_eq!(page.utm_term.chars().count(), MAX_UTM_CHARS);
    }

    #[test]
    fn a_referrer_keeps_its_path_only() {
        assert_eq!(
            clean_referrer("https://www.google.com/search?q=private+words#frag"),
            "https://www.google.com/search"
        );
        assert_eq!(
            clean_referrer("https://old.example/-/auth/login?username=a&password=b"),
            "https://old.example/-/auth/login"
        );
        assert_eq!(
            clean_referrer("android-app://com.example/path?x=1"),
            "android-app://com.example/path"
        );
        assert_eq!(clean_referrer(""), "");
        assert_eq!(parse_referrer_source(&clean_referrer("")), "Direct");
    }
}
