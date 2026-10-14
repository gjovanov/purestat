//! Sign-in with Google and GitHub (docs/api.md#oauth). The rules are in
//! `purestat_services::oauth`; this is the HTTP around them.
//!
//! The browser goes `GET /api/oauth/{provider}` → the provider →
//! `GET /api/oauth/{provider}/callback` → the SPA. No credential is ever in an
//! address: the callback sets the same HttpOnly session cookies as a password
//! login and redirects to a fixed SPA page, `/auth/oauth`, which loads the
//! session from `GET /api/me`. A refusal redirects to
//! `/login?oauth_error=<code>`, a code and nothing else.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use purestat_services::oauth::{
    self, FLOW_TTL_SECS, FlowFailure, OAuthFlows, Provider, RedisFlowStore, Resolution,
};
use serde::Deserialize;

use crate::error::ApiError;
use crate::routes::auth::set_auth_cookies;
use crate::state::AppState;

/// The flow cookie under https. The `__Host-` prefix makes a browser refuse
/// it from a sibling subdomain or without `Secure`, so no other site under
/// the same parent domain can plant its own flow in a visitor's browser.
const FLOW_COOKIE_SECURE: &str = "__Host-purestat_oauth";

/// The flow cookie over plain http (local development), which `__Host-`
/// does not allow.
const FLOW_COOKIE_PLAIN: &str = "purestat_oauth";

/// The SPA page a finished sign-in lands on.
const SIGNED_IN_PATH: &str = "/auth/oauth";

#[derive(Debug, Deserialize)]
pub struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    /// Set instead of `code` when the provider did not sign the person in,
    /// `access_denied` when they declined.
    error: Option<String>,
}

/// `GET /api/auth/providers`: the providers this server signs in with, e.g.
/// `["google","github"]`, and `[]` when OAuth is off. Names only, so the SPA
/// shows a button for a provider only when it works.
pub async fn providers(State(state): State<AppState>) -> Json<Vec<&'static str>> {
    Json(
        state
            .oauth
            .providers()
            .into_iter()
            .map(Provider::slug)
            .collect(),
    )
}

/// `GET /api/oauth/{provider}`: start a sign-in. Sets the flow cookie and
/// sends the browser to the provider with a fresh `state` and PKCE challenge.
pub async fn oauth_redirect(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Response, ApiError> {
    let provider = available_provider(&state, &slug)?;
    let flows = OAuthFlows::new(RedisFlowStore::new(state.redis.clone()));
    let begun = match flows.begin(provider, now()).await {
        Ok(begun) => begun,
        Err(e) => {
            tracing::warn!(provider = provider.slug(), error = %e, "OAuth: could not start a sign-in");
            return Ok(refuse(&state, "unavailable"));
        }
    };
    let location = state
        .oauth
        .authorize_url(provider, &begun.state, &begun.code_challenge)
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    let secure = state.settings.app.secure_cookies();
    Ok(redirect(&location, flow_cookie(secure, &begun.flow_id)))
}

/// `GET /api/oauth/{provider}/callback`: finish a sign-in.
///
/// In this order, each step refusing before the next: the flow this browser
/// started is consumed and its `state` checked; the provider's answer; the
/// code exchange with the PKCE verifier; who signed in; then, and only then,
/// the account.
pub async fn oauth_callback(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    headers: HeaderMap,
    Query(query): Query<CallbackQuery>,
) -> Result<Response, ApiError> {
    let provider = available_provider(&state, &slug)?;
    // One rule for every cookie the API sets (AppSettings::secure_cookies).
    let secure = state.settings.app.secure_cookies();

    // 1. The flow: used once, whatever happens below.
    let flows = OAuthFlows::new(RedisFlowStore::new(state.redis.clone()));
    let flow_id = cookie_value(&headers, flow_cookie_name(secure));
    let verifier = match flows
        .finish(flow_id.as_deref(), provider, query.state.as_deref(), now())
        .await
    {
        Ok(verifier) => verifier,
        Err(FlowFailure::Refused(why)) => {
            tracing::info!(provider = provider.slug(), reason = ?why, "OAuth: callback refused: state");
            return Ok(refuse(&state, "state"));
        }
        Err(FlowFailure::Unavailable(e)) => {
            tracing::warn!(provider = provider.slug(), error = %e, "OAuth: callback refused: flow store unavailable");
            return Ok(refuse(&state, "unavailable"));
        }
    };

    // 2. The provider's answer.
    if let Some(error) = query.error.as_deref() {
        tracing::info!(
            provider = provider.slug(),
            error,
            "OAuth: the provider signed nobody in"
        );
        let code = if error == "access_denied" {
            "cancelled"
        } else {
            "provider"
        };
        return Ok(refuse(&state, code));
    }
    let Some(code) = query.code.as_deref().filter(|c| !c.is_empty()) else {
        return Ok(refuse(&state, "provider"));
    };

    // 3. The code exchange, and who signed in.
    let identity = match state.oauth.exchange_code(provider, code, &verifier).await {
        Ok(token) => state.oauth.identity(provider, &token).await,
        Err(e) => Err(e),
    };
    let identity = match identity {
        Ok(identity) => identity,
        Err(e) => {
            tracing::warn!(provider = provider.slug(), error = %e, "OAuth: callback refused: provider");
            return Ok(refuse(&state, "provider"));
        }
    };

    // 4. The account.
    let resolution = match oauth::resolve(&state.users, &identity, state.email.is_some()).await {
        Ok(resolution) => resolution,
        Err(e) => {
            tracing::error!(provider = provider.slug(), error = %e, "OAuth: account lookup failed");
            return Ok(refuse(&state, "server"));
        }
    };
    let outcome = resolution.outcome();
    let user = match resolution {
        Resolution::SignedIn(user) | Resolution::Linked(user) | Resolution::Created(user) => user,
        Resolution::Refused(why) => {
            tracing::info!(
                provider = provider.slug(),
                outcome,
                "OAuth: sign-in refused"
            );
            return Ok(refuse(&state, why.code()));
        }
    };
    let Some(user_id) = user.id.map(|id| id.to_hex()) else {
        return Ok(refuse(&state, "server"));
    };
    tracing::info!(provider = provider.slug(), user_id = %user_id, outcome, "OAuth: signed in");

    // 5. The session, as a password login sets it (Secure under https). The
    //    address bar gets none of it.
    let tokens = state
        .auth
        .generate_access_token(&user_id, &user.email, &user.username)
        .and_then(|access| {
            state
                .auth
                .generate_refresh_token(&user_id, &user.email, &user.username)
                .map(|refresh| (access, refresh))
        });
    let Ok((access, refresh)) = tokens else {
        tracing::error!(user_id = %user_id, "OAuth: could not sign a session token");
        return Ok(refuse(&state, "server"));
    };
    let mut response = redirect(
        &format!("{}{SIGNED_IN_PATH}", state.oauth.public_url()),
        clear_flow_cookie(secure),
    );
    set_auth_cookies(response.headers_mut(), &access, &refresh, secure);
    Ok(response)
}

/// The provider a path names, when this server signs in with it. Every other
/// name is a 404: the three providers that were listed once say why.
fn available_provider(state: &AppState, slug: &str) -> Result<Provider, ApiError> {
    if let Some(name) = oauth::retired_provider(slug) {
        return Err(ApiError::NotFound(format!(
            "Sign-in with {name} is not supported. Purestat signs in with Google or GitHub, \
             or with an email address and password."
        )));
    }
    let provider = Provider::from_slug(slug)
        .ok_or_else(|| ApiError::NotFound("Unknown sign-in provider.".to_string()))?;
    if !state.oauth.is_available(provider) {
        return Err(ApiError::NotFound(format!(
            "Sign-in with {} is not configured on this server.",
            provider.name()
        )));
    }
    Ok(provider)
}

/// Back to the sign-in page with the reason, and the flow cookie cleared.
fn refuse(state: &AppState, code: &str) -> Response {
    let location = format!("{}/login?oauth_error={code}", state.oauth.public_url());
    redirect(
        &location,
        clear_flow_cookie(state.settings.app.secure_cookies()),
    )
}

/// A 303 to `location` that sets `cookie` and is never cached.
fn redirect(location: &str, cookie: Option<HeaderValue>) -> Response {
    let mut response = StatusCode::SEE_OTHER.into_response();
    let headers = response.headers_mut();
    if let Ok(value) = HeaderValue::from_str(location) {
        headers.insert(header::LOCATION, value);
    }
    if let Some(cookie) = cookie {
        headers.append(header::SET_COOKIE, cookie);
    }
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

fn flow_cookie_name(secure: bool) -> &'static str {
    if secure {
        FLOW_COOKIE_SECURE
    } else {
        FLOW_COOKIE_PLAIN
    }
}

/// The flow cookie: HttpOnly, SameSite=Lax (a provider's redirect back is a
/// cross-site navigation, which Lax lets through and Strict would not), gone
/// after the flow's lifetime. Under https, `Secure` and `__Host-` (path `/`);
/// over http, scoped to `/api/oauth`.
fn flow_cookie(secure: bool, flow_id: &str) -> Option<HeaderValue> {
    let attributes = if secure {
        "Path=/; HttpOnly; Secure; SameSite=Lax"
    } else {
        "Path=/api/oauth; HttpOnly; SameSite=Lax"
    };
    let name = flow_cookie_name(secure);
    HeaderValue::from_str(&format!(
        "{name}={flow_id}; Max-Age={FLOW_TTL_SECS}; {attributes}"
    ))
    .ok()
}

fn clear_flow_cookie(secure: bool) -> Option<HeaderValue> {
    let attributes = if secure {
        "Path=/; HttpOnly; Secure; SameSite=Lax"
    } else {
        "Path=/api/oauth; HttpOnly; SameSite=Lax"
    };
    let name = flow_cookie_name(secure);
    HeaderValue::from_str(&format!("{name}=; Max-Age=0; {attributes}")).ok()
}

/// The value of cookie `name`, matched exactly: `x_purestat_oauth` is not
/// `purestat_oauth`. Reads every `Cookie` header, as HTTP/2 may split them.
fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(key, _)| *key == name)
        .map(|(_, value)| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cookies(raw: &[&str]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for value in raw {
            headers.append(header::COOKIE, HeaderValue::from_str(value).unwrap());
        }
        headers
    }

    #[test]
    fn the_flow_cookie_is_bound_to_the_browser_and_short_lived() {
        let prod = flow_cookie(true, "abc").unwrap();
        let prod = prod.to_str().unwrap();
        assert_eq!(
            prod,
            "__Host-purestat_oauth=abc; Max-Age=600; Path=/; HttpOnly; Secure; SameSite=Lax"
        );
        let dev = flow_cookie(false, "abc").unwrap();
        let dev = dev.to_str().unwrap();
        assert_eq!(
            dev,
            "purestat_oauth=abc; Max-Age=600; Path=/api/oauth; HttpOnly; SameSite=Lax"
        );
        assert!(!dev.contains("Secure"));
    }

    #[test]
    fn clearing_the_flow_cookie_matches_its_name_and_path() {
        assert_eq!(
            clear_flow_cookie(true).unwrap().to_str().unwrap(),
            "__Host-purestat_oauth=; Max-Age=0; Path=/; HttpOnly; Secure; SameSite=Lax"
        );
        assert_eq!(
            clear_flow_cookie(false).unwrap().to_str().unwrap(),
            "purestat_oauth=; Max-Age=0; Path=/api/oauth; HttpOnly; SameSite=Lax"
        );
    }

    #[test]
    fn a_cookie_is_found_by_its_exact_name_in_any_cookie_header() {
        let h = cookies(&[
            "theme=dark; x_purestat_oauth=nope",
            "purestat_oauth=yes; a=b",
        ]);
        assert_eq!(cookie_value(&h, "purestat_oauth").as_deref(), Some("yes"));
        let h = cookies(&["purestat_oauth_v2=nope; xpurestat_oauth=nope"]);
        assert_eq!(cookie_value(&h, "purestat_oauth"), None);
        let h = cookies(&["purestat_oauth=", "__Host-purestat_oauth=v"]);
        assert_eq!(cookie_value(&h, "purestat_oauth"), None);
        assert_eq!(
            cookie_value(&h, "__Host-purestat_oauth").as_deref(),
            Some("v")
        );
        assert_eq!(cookie_value(&HeaderMap::new(), "purestat_oauth"), None);
    }
}
