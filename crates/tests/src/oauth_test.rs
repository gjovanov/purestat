//! PS6: sign-in with Google and GitHub, end to end through the real router,
//! MongoDB and Redis, with the providers played by a mock on 127.0.0.1. No
//! request leaves the machine.
//!
//! Each test runs its own API in-process (`purestat_api::build_router`) on a
//! database of its own, with OAuth on and both providers pointed at the mock,
//! and drives it as a browser would: the redirect, the provider (the mock
//! issues a code bound to the PKCE challenge the browser brought), the
//! callback, `GET /api/me` with the cookie. The mock checks a code exchange
//! the way Google and GitHub do: the client credentials, the callback
//! address, a code used once, and a verifier whose S256 is the challenge.
//!
//! Needs MongoDB and Redis, from the PURESTAT__* settings. Not the API, not
//! ClickHouse:
//!   cargo test -p purestat-tests oauth_test

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use axum::extract::{Form, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use bson::doc;
use bson::oid::ObjectId;
use purestat_api::state::AppState;
use purestat_config::{OAuthProviderSettings, OAuthSettings, Settings};
use purestat_db::models::{OAuthProvider, User};
use purestat_services::UserDao;
use purestat_services::oauth::{OAuthClient, Provider, ProviderEndpoints, code_challenge};
use reqwest::Url;
use serde_json::{Value, json};

const GOOGLE_ID: &str = "fake-google-client-id";
const GOOGLE_SECRET: &str = "fake-google-client-secret";
const GITHUB_ID: &str = "fake-github-client-id";
const GITHUB_SECRET: &str = "fake-github-client-secret";

/// The public address the test API believes it has: https, so the flow
/// cookie is the production one (`__Host-`, Secure).
const PUBLIC_URL: &str = "https://purestat.test";
const FLOW_COOKIE: &str = "__Host-purestat_oauth";

fn signed_in_url() -> String {
    format!("{PUBLIC_URL}/auth/oauth")
}

fn uid() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

fn unique_email(tag: &str) -> String {
    format!("{tag}-{}@purestat.test", &uid()[..12])
}

// ---------------------------------------------------------------------------
// The providers
// ---------------------------------------------------------------------------

/// Someone signed in at a provider.
#[derive(Clone, Debug)]
enum Person {
    Google {
        sub: String,
        email: String,
        verified: bool,
    },
    GitHub {
        id: u64,
        login: String,
        /// The free-text profile address, which purestat must never use.
        public_email: Option<String>,
        emails: Value,
    },
}

impl Person {
    fn google(email: &str, verified: bool) -> Self {
        Person::Google {
            sub: format!("fake-google-sub-{}", uid()),
            email: email.to_string(),
            verified,
        }
    }

    fn github(login: &str, public_email: Option<&str>, emails: Value) -> Self {
        Person::GitHub {
            id: (uuid::Uuid::new_v4().as_u128() as u64 & 0xFFFF_FFFF) + 1,
            login: login.to_string(),
            public_email: public_email.map(str::to_string),
            emails,
        }
    }

    fn identity(&self) -> OAuthProvider {
        match self {
            Person::Google { sub, .. } => OAuthProvider {
                provider: "google".into(),
                provider_id: sub.clone(),
            },
            Person::GitHub { id, .. } => OAuthProvider {
                provider: "github".into(),
                provider_id: id.to_string(),
            },
        }
    }
}

#[derive(Default)]
struct MockState {
    /// code → (the PKCE challenge it is bound to, who signed in)
    codes: HashMap<String, (String, Person)>,
    /// access token → who
    tokens: HashMap<String, Person>,
    token_requests: usize,
}

type Shared = Arc<Mutex<MockState>>;

#[derive(Clone)]
struct Mock {
    url: String,
    state: Shared,
}

impl Mock {
    async fn start() -> Self {
        let state = Shared::default();
        let app = Router::new()
            .route("/google/token", post(google_token))
            .route("/google/userinfo", get(google_userinfo))
            .route("/github/token", post(github_token))
            .route("/github/user", get(github_user))
            .route("/github/user/emails", get(github_emails))
            .with_state(state.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Mock { url, state }
    }

    fn endpoints(&self, provider: &str) -> ProviderEndpoints {
        let profile = if provider == "google" {
            "userinfo"
        } else {
            "user"
        };
        ProviderEndpoints {
            authorize: format!("{}/{provider}/authorize", self.url),
            token: format!("{}/{provider}/token", self.url),
            profile: format!("{}/{provider}/{profile}", self.url),
            emails: format!("{}/{provider}/user/emails", self.url),
        }
    }

    /// The provider signs `person` in: a code bound to the challenge the
    /// browser arrived with.
    fn issue_code(&self, challenge: &str, person: Person) -> String {
        let code = format!("fake-code-{}", uid());
        self.state
            .lock()
            .unwrap()
            .codes
            .insert(code.clone(), (challenge.to_string(), person));
        code
    }

    fn token_requests(&self) -> usize {
        self.state.lock().unwrap().token_requests
    }
}

/// A code exchange, checked as the providers check it.
fn exchange(
    state: &Shared,
    provider: &str,
    form: &HashMap<String, String>,
) -> Result<String, &'static str> {
    let mut s = state.lock().unwrap();
    s.token_requests += 1;
    let field = |name: &str| form.get(name).map(String::as_str);
    let (id, secret) = if provider == "google" {
        (GOOGLE_ID, GOOGLE_SECRET)
    } else {
        (GITHUB_ID, GITHUB_SECRET)
    };
    if field("client_id") != Some(id) || field("client_secret") != Some(secret) {
        return Err("invalid_client");
    }
    let callback = format!("{PUBLIC_URL}/api/oauth/{provider}/callback");
    if field("redirect_uri") != Some(callback.as_str()) {
        return Err("redirect_uri_mismatch");
    }
    if provider == "google" && field("grant_type") != Some("authorization_code") {
        return Err("unsupported_grant_type");
    }
    // A code is good once.
    let (challenge, person) = s
        .codes
        .remove(field("code").ok_or("invalid_request")?)
        .ok_or("invalid_grant")?;
    // PKCE: the verifier must be the one whose S256 the browser carried.
    match field("code_verifier") {
        Some(verifier) if code_challenge(verifier) == challenge => {}
        _ => return Err("invalid_grant"),
    }
    let token = format!("fake-access-token-{}", uid());
    s.tokens.insert(token.clone(), person);
    Ok(token)
}

fn person_for(state: &Shared, headers: &HeaderMap) -> Option<Person> {
    let token = headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")?;
    state.lock().unwrap().tokens.get(token).cloned()
}

async fn google_token(
    State(state): State<Shared>,
    Form(form): Form<HashMap<String, String>>,
) -> Response {
    match exchange(&state, "google", &form) {
        Ok(token) => Json(json!({
            "access_token": token,
            "token_type": "Bearer",
            "expires_in": 3599,
            "scope": "openid email profile",
        }))
        .into_response(),
        Err(error) => (StatusCode::BAD_REQUEST, Json(json!({ "error": error }))).into_response(),
    }
}

async fn google_userinfo(State(state): State<Shared>, headers: HeaderMap) -> Response {
    match person_for(&state, &headers) {
        Some(Person::Google {
            sub,
            email,
            verified,
        }) => Json(json!({
            "sub": sub,
            "email": email,
            "email_verified": verified,
            "name": "Dana Example",
        }))
        .into_response(),
        _ => StatusCode::UNAUTHORIZED.into_response(),
    }
}

/// GitHub answers in form encoding unless asked for JSON, and a failed
/// exchange with 200.
async fn github_token(
    State(state): State<Shared>,
    headers: HeaderMap,
    Form(form): Form<HashMap<String, String>>,
) -> Response {
    let result = exchange(&state, "github", &form);
    let wants_json =
        headers.get(header::ACCEPT).and_then(|v| v.to_str().ok()) == Some("application/json");
    match (result, wants_json) {
        (Ok(token), true) => Json(json!({
            "access_token": token,
            "scope": "user:email",
            "token_type": "bearer",
        }))
        .into_response(),
        (Err(error), true) => Json(json!({
            "error": error,
            "error_description": "refused by the mock",
        }))
        .into_response(),
        (Ok(token), false) => {
            format!("access_token={token}&scope=user%3Aemail&token_type=bearer").into_response()
        }
        (Err(error), false) => format!("error={error}").into_response(),
    }
}

/// GitHub refuses an API request without a User-Agent.
fn github_refuses(headers: &HeaderMap) -> bool {
    headers.get(header::USER_AGENT).is_none()
}

async fn github_user(State(state): State<Shared>, headers: HeaderMap) -> Response {
    if github_refuses(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    match person_for(&state, &headers) {
        Some(Person::GitHub {
            id,
            login,
            public_email,
            ..
        }) => Json(json!({ "id": id, "login": login, "name": null, "email": public_email }))
            .into_response(),
        _ => StatusCode::UNAUTHORIZED.into_response(),
    }
}

async fn github_emails(State(state): State<Shared>, headers: HeaderMap) -> Response {
    if github_refuses(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    match person_for(&state, &headers) {
        Some(Person::GitHub { emails, .. }) => Json(emails).into_response(),
        _ => StatusCode::UNAUTHORIZED.into_response(),
    }
}

// ---------------------------------------------------------------------------
// The API under test
// ---------------------------------------------------------------------------

/// How an existing password account came to be activated, if it was.
#[derive(Clone, Copy, Debug)]
enum Proof {
    /// Registered and never activated.
    Never,
    /// Activated by the emailed link.
    EmailedLink,
    /// Activated at registration because no email service is configured.
    NoEmailService,
    /// Activated before purestat recorded how (`email_proven` absent).
    Legacy,
}

struct Harness {
    api: String,
    http: reqwest::Client,
    mock: Mock,
    db: mongodb::Database,
    users: UserDao,
}

struct Started {
    /// `name=value`, as the browser would send it back.
    cookie: String,
    /// The whole Set-Cookie line.
    set_cookie: String,
    authorize: Url,
    state: String,
    challenge: String,
}

impl Started {
    fn query(&self) -> HashMap<String, String> {
        self.authorize.query_pairs().into_owned().collect()
    }
}

/// Where a callback sent the browser, and the cookies it set.
#[derive(Debug)]
struct Landing {
    status: u16,
    location: String,
    cookies: Vec<String>,
}

impl Landing {
    /// The session cookie, `access_token=…`, when one was set.
    fn session(&self) -> Option<String> {
        self.cookies
            .iter()
            .find(|c| c.starts_with("access_token=") && !c.starts_with("access_token=;"))
            .map(|c| c.split(';').next().unwrap().to_string())
    }

    /// The code of a refusal (`/login?oauth_error=<code>`).
    fn refusal(&self) -> Option<String> {
        let prefix = format!("{PUBLIC_URL}/login?oauth_error=");
        self.location.strip_prefix(&prefix).map(str::to_string)
    }
}

fn set_cookies(response: &reqwest::Response) -> Vec<String> {
    response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .map(|v| v.to_str().unwrap().to_string())
        .collect()
}

fn location(response: &reqwest::Response) -> String {
    response
        .headers()
        .get(header::LOCATION)
        .map(|v| v.to_str().unwrap().to_string())
        .unwrap_or_default()
}

impl Harness {
    /// An API with OAuth on, Google and GitHub pointed at a fresh mock, on a
    /// database of its own. `configure` changes the settings first.
    async fn start(configure: impl FnOnce(&mut Settings)) -> Self {
        let mut settings = Settings::load().expect("PURESTAT__* settings");
        settings.database.name = format!("purestat_oauth_test_{}", &uid()[..16]);
        settings.app.frontend_url = PUBLIC_URL.to_string();
        // Signs this test API's tokens only; made fresh, never written down.
        settings.jwt.secret = format!("{}{}", uid(), uid());
        settings.rate_limit.enabled = false;
        settings.email.api_key = String::new();
        settings.oauth = OAuthSettings {
            enabled: true,
            google: Some(OAuthProviderSettings {
                client_id: GOOGLE_ID.to_string(),
                client_secret: GOOGLE_SECRET.to_string(),
            }),
            github: Some(OAuthProviderSettings {
                client_id: GITHUB_ID.to_string(),
                client_secret: GITHUB_SECRET.to_string(),
            }),
        };
        configure(&mut settings);

        let mock = Mock::start().await;
        let db = purestat_db::connect(&settings).await.expect("MongoDB");
        purestat_db::indexes::ensure_indexes(&db)
            .await
            .expect("indexes");
        let ch = purestat_db::clickhouse::connection::connect(&settings);
        let redis = redis::aio::ConnectionManager::new(
            redis::Client::open(settings.redis.url.as_str()).expect("a Redis URL"),
        )
        .await
        .expect("Redis");
        let mut state = AppState::new(db.clone(), ch, redis, settings.clone())
            .await
            .expect("app state");
        state.oauth = Arc::new(
            OAuthClient::from_settings(&settings.oauth, &settings.app.frontend_url)
                .with_endpoints(Provider::Google, mock.endpoints("google"))
                .with_endpoints(Provider::GitHub, mock.endpoints("github")),
        );
        let app = purestat_api::build_router(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let api = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await
            .unwrap()
        });

        Harness {
            api,
            // A browser that does not follow redirects, so each hop is seen,
            // and keeps no cookie jar: every cookie sent is one a test chose.
            http: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .unwrap(),
            mock,
            users: UserDao::new(&db),
            db,
        }
    }

    async fn finish(self) {
        let _ = self.db.drop().await;
    }

    async fn get(&self, path: &str, cookie: Option<&str>) -> reqwest::Response {
        let mut request = self.http.get(format!("{}{path}", self.api));
        if let Some(cookie) = cookie {
            request = request.header(header::COOKIE, cookie);
        }
        request.send().await.expect("request")
    }

    async fn providers(&self) -> Value {
        let response = self.get("/api/auth/providers", None).await;
        assert_eq!(response.status(), 200);
        response.json().await.unwrap()
    }

    /// `GET /api/oauth/{provider}`, as the sign-in button does.
    async fn start_sign_in(&self, provider: &str) -> Started {
        let response = self.get(&format!("/api/oauth/{provider}"), None).await;
        if response.status() != 303 {
            panic!(
                "GET /api/oauth/{provider}: {} {}",
                response.status(),
                response.text().await.unwrap_or_default()
            );
        }
        let set_cookie = set_cookies(&response)
            .into_iter()
            .find(|c| c.starts_with(&format!("{FLOW_COOKIE}=")))
            .expect("a flow cookie");
        let cookie = set_cookie.split(';').next().unwrap().to_string();
        let authorize = Url::parse(&location(&response)).expect("a provider address");
        let query: HashMap<String, String> = authorize.query_pairs().into_owned().collect();
        Started {
            cookie,
            set_cookie,
            state: query.get("state").cloned().expect("a state"),
            challenge: query
                .get("code_challenge")
                .cloned()
                .expect("a PKCE challenge"),
            authorize,
        }
    }

    /// The provider's redirect back, with `query` and the browser's `cookie`.
    async fn callback(
        &self,
        provider: &str,
        query: &[(&str, &str)],
        cookie: Option<&str>,
    ) -> Landing {
        let mut url = Url::parse(&format!("{}/api/oauth/{provider}/callback", self.api)).unwrap();
        url.query_pairs_mut().extend_pairs(query);
        let mut request = self.http.get(url);
        if let Some(cookie) = cookie {
            request = request.header(header::COOKIE, cookie);
        }
        let response = request.send().await.expect("callback");
        Landing {
            status: response.status().as_u16(),
            location: location(&response),
            cookies: set_cookies(&response),
        }
    }

    /// A whole sign-in: the button, the provider signing `person` in, the
    /// callback.
    async fn sign_in(&self, provider: &str, person: Person) -> Landing {
        let started = self.start_sign_in(provider).await;
        let code = self.mock.issue_code(&started.challenge, person);
        self.callback(
            provider,
            &[("code", &code), ("state", &started.state)],
            Some(&started.cookie),
        )
        .await
    }

    /// `GET /api/me` with the session cookie, as the SPA loads the session.
    async fn me(&self, session: &str) -> Value {
        let response = self.get("/api/me", Some(session)).await;
        assert_eq!(
            response.status(),
            200,
            "GET /api/me with the session cookie"
        );
        response.json().await.unwrap()
    }

    async fn user(&self, email: &str) -> Option<User> {
        self.users
            .base
            .collection()
            .find_one(doc! { "email": email })
            .await
            .unwrap()
    }

    async fn user_by_id(&self, id: ObjectId) -> User {
        self.users.base.find_by_id(id).await.unwrap()
    }

    async fn count(&self, email: &str) -> u64 {
        self.users
            .base
            .count(doc! { "email": email })
            .await
            .unwrap()
    }

    /// A password account for `email`, activated as `proof` says.
    async fn password_account(&self, email: &str, proof: Proof) -> ObjectId {
        let username = format!("pw-{}", &uid()[..12]);
        if let Proof::Legacy = proof {
            // As an activated account from before `email_proven` looks.
            let now = bson::DateTime::now();
            let users = self.db.collection::<bson::Document>("users");
            let inserted = users
                .insert_one(doc! {
                    "email": email,
                    "username": username,
                    "display_name": "Legacy",
                    "avatar": null,
                    "password_hash": "$argon2id$fake",
                    "oauth_providers": [],
                    "is_verified": true,
                    "locale": "en",
                    "created_at": now,
                    "updated_at": now,
                    "deleted_at": null,
                })
                .await
                .unwrap();
            return inserted.inserted_id.as_object_id().unwrap();
        }
        let user = self
            .users
            .create(
                email.to_string(),
                username,
                "Pat".to_string(),
                Some("$argon2id$fake".to_string()),
            )
            .await
            .unwrap();
        let id = user.id.unwrap();
        match proof {
            Proof::Never | Proof::Legacy => {}
            Proof::EmailedLink => {
                self.users.activate(id, true).await.unwrap();
            }
            Proof::NoEmailService => {
                self.users.activate(id, false).await.unwrap();
            }
        }
        id
    }
}

// ---------------------------------------------------------------------------
// The tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn google_creates_a_verified_account_and_the_session_is_only_in_cookies() {
    let h = Harness::start(|_| {}).await;
    let email = unique_email("google-new");

    // The redirect: a fresh state and an S256 challenge, and a flow cookie
    // that binds them to this browser.
    let started = h.start_sign_in("google").await;
    assert_eq!(
        started.authorize.as_str().split('?').next().unwrap(),
        format!("{}/google/authorize", h.mock.url)
    );
    let q = started.query();
    assert_eq!(q["client_id"], GOOGLE_ID);
    assert_eq!(
        q["redirect_uri"],
        format!("{PUBLIC_URL}/api/oauth/google/callback")
    );
    assert_eq!(q["response_type"], "code");
    assert_eq!(q["scope"], "openid email profile");
    assert_eq!(q["code_challenge_method"], "S256");
    assert_eq!(started.state.len(), 43);
    assert_eq!(started.challenge.len(), 43);
    for attribute in [
        "Max-Age=600",
        "Path=/",
        "HttpOnly",
        "Secure",
        "SameSite=Lax",
    ] {
        assert!(
            started.set_cookie.contains(attribute),
            "{}",
            started.set_cookie
        );
    }
    assert!(
        !started.set_cookie.contains(&started.state),
        "the cookie is not the state"
    );
    assert!(!started.authorize.as_str().contains(GOOGLE_SECRET));

    // The provider signs Dana in; the browser comes back.
    let person = Person::google(&email, true);
    let code = h.mock.issue_code(&started.challenge, person.clone());
    let landing = h
        .callback(
            "google",
            &[("code", &code), ("state", &started.state)],
            Some(&started.cookie),
        )
        .await;
    assert_eq!(landing.status, 303);
    // A fixed SPA address: no token, no code, no user id in it.
    assert_eq!(landing.location, signed_in_url());
    let session = landing.session().expect("a session cookie");
    for name in ["access_token=", "refresh_token="] {
        let cookie = landing
            .cookies
            .iter()
            .find(|c| c.starts_with(name))
            .unwrap_or_else(|| panic!("{name} is set"));
        assert!(cookie.contains("HttpOnly"), "{cookie}");
        let value = cookie.split(';').next().unwrap().trim_start_matches(name);
        assert!(!landing.location.contains(value));
    }
    assert!(
        landing
            .cookies
            .iter()
            .any(|c| c.starts_with(&format!("{FLOW_COOKIE}=;")) && c.contains("Max-Age=0")),
        "the flow cookie is cleared: {:?}",
        landing.cookies
    );
    // The exchange carried the right verifier: the mock refuses any other.
    assert_eq!(h.mock.token_requests(), 1);

    // The account: verified, its address proven, no password, the identity
    // linked.
    let user = h.user(&email).await.expect("an account was created");
    assert!(user.is_verified);
    assert_eq!(user.email_proven, Some(true));
    assert!(user.password_hash.is_none());
    assert_eq!(user.oauth_providers, vec![person.identity()]);

    // The SPA loads the session from the cookie.
    let me = h.me(&session).await;
    assert_eq!(me["email"], email.as_str());
    assert_eq!(me["id"], user.id.unwrap().to_hex().as_str());

    // The same callback again: the flow is spent.
    let replay = h
        .callback(
            "google",
            &[("code", &code), ("state", &started.state)],
            Some(&started.cookie),
        )
        .await;
    assert_eq!(replay.refusal().as_deref(), Some("state"));
    assert!(replay.session().is_none());
    assert_eq!(
        h.mock.token_requests(),
        1,
        "a refused state never reaches the provider"
    );
    assert_eq!(h.count(&email).await, 1);

    // The account has no password, and the password form says no more about
    // it than about an address nobody holds.
    let attempt = format!("not-it-{}", uid());
    for address in [email.clone(), unique_email("nobody")] {
        let response = h
            .http
            .post(format!("{}/api/auth/login", h.api))
            .json(&json!({ "email": address, "password": attempt }))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 401);
        let body: Value = response.json().await.unwrap();
        assert_eq!(body["message"], "Invalid credentials", "{address}");
    }
    h.finish().await;
}

#[tokio::test]
async fn a_callback_this_browser_did_not_start_is_refused_before_any_account_work() {
    let h = Harness::start(|_| {}).await;
    let email = unique_email("csrf");

    // The attacker's own flow, finished at the provider: a valid code and
    // state, both theirs.
    let attacker = h.start_sign_in("google").await;
    let code = h
        .mock
        .issue_code(&attacker.challenge, Person::google(&email, true));

    // The victim's browser has a flow of its own, or none.
    let victim = h.start_sign_in("google").await;
    for (browser, cookie) in [
        ("with its own flow", Some(victim.cookie.as_str())),
        ("with no flow", None),
    ] {
        let landing = h
            .callback(
                "google",
                &[("code", &code), ("state", &attacker.state)],
                cookie,
            )
            .await;
        assert_eq!(landing.status, 303);
        assert_eq!(landing.refusal().as_deref(), Some("state"), "{browser}");
        assert!(landing.session().is_none(), "{browser}");
    }
    // The right cookie, but no state at all.
    let third = h.start_sign_in("google").await;
    let landing = h
        .callback("google", &[("code", &code)], Some(&third.cookie))
        .await;
    assert_eq!(landing.refusal().as_deref(), Some("state"));
    // A flow started for GitHub does not finish at Google's callback.
    let github = h.start_sign_in("github").await;
    let landing = h
        .callback(
            "google",
            &[("code", &code), ("state", &github.state)],
            Some(&github.cookie),
        )
        .await;
    assert_eq!(landing.refusal().as_deref(), Some("state"));

    // None of it reached the provider or touched an account.
    assert_eq!(h.mock.token_requests(), 0);
    assert!(h.user(&email).await.is_none());

    // The forged callback spent the victim's flow: it cannot be finished now.
    let late = h
        .callback(
            "google",
            &[("code", &code), ("state", &victim.state)],
            Some(&victim.cookie),
        )
        .await;
    assert_eq!(late.refusal().as_deref(), Some("state"));
    h.finish().await;
}

#[tokio::test]
async fn a_code_bound_to_another_pkce_challenge_is_refused() {
    // The control for the mock's PKCE check: it can fail, so its passes in
    // the other tests mean the right verifier was sent.
    let h = Harness::start(|_| {}).await;
    let email = unique_email("pkce");
    let started = h.start_sign_in("google").await;
    let elsewhere = code_challenge("a-verifier-this-server-never-saw-0123456789abcd");
    let code = h.mock.issue_code(&elsewhere, Person::google(&email, true));
    let landing = h
        .callback(
            "google",
            &[("code", &code), ("state", &started.state)],
            Some(&started.cookie),
        )
        .await;
    assert_eq!(landing.refusal().as_deref(), Some("provider"));
    assert_eq!(h.mock.token_requests(), 1);
    assert!(h.user(&email).await.is_none());
    h.finish().await;
}

#[tokio::test]
async fn declining_at_the_provider_is_reported_as_cancelled() {
    let h = Harness::start(|_| {}).await;
    let started = h.start_sign_in("github").await;
    let landing = h
        .callback(
            "github",
            &[("error", "access_denied"), ("state", &started.state)],
            Some(&started.cookie),
        )
        .await;
    assert_eq!(landing.refusal().as_deref(), Some("cancelled"));
    assert_eq!(h.mock.token_requests(), 0);
    h.finish().await;
}

#[tokio::test]
async fn google_without_a_verified_address_is_refused() {
    let h = Harness::start(|_| {}).await;
    let email = unique_email("google-unverified");
    let landing = h.sign_in("google", Person::google(&email, false)).await;
    assert_eq!(landing.refusal().as_deref(), Some("email_unverified"));
    assert!(landing.session().is_none());
    assert!(h.user(&email).await.is_none());
    h.finish().await;
}

#[tokio::test]
async fn github_signs_in_with_the_primary_verified_address_only() {
    let h = Harness::start(|_| {}).await;

    // Primary and verified: a new account, named after the login.
    let login = format!("Octo-{}", &uid()[..8]);
    let email = unique_email("gh-primary");
    let landing = h
        .sign_in(
            "github",
            Person::github(
                &login,
                None,
                json!([
                    { "email": unique_email("gh-secondary"), "primary": false, "verified": true, "visibility": null },
                    { "email": email, "primary": true, "verified": true, "visibility": "private" },
                ]),
            ),
        )
        .await;
    assert_eq!(landing.location, signed_in_url(), "{landing:?}");
    let user = h.user(&email).await.expect("an account");
    assert_eq!(user.username, login.to_lowercase());
    assert!(user.is_verified);
    assert_eq!(user.email_proven, Some(true));
    assert_eq!(user.oauth_providers[0].provider, "github");

    // An account that proved its address, which GitHub's free-text profile
    // address names: it must not be the basis of anything.
    let held = unique_email("gh-held");
    let holder = h.password_account(&held, Proof::EmailedLink).await;

    // Primary but unverified, with a verified secondary: refused.
    let primary_unverified = unique_email("gh-primary-unverified");
    let secondary = unique_email("gh-verified-secondary");
    let landing = h
        .sign_in(
            "github",
            Person::github(
                &format!("u-{}", &uid()[..8]),
                Some(&held),
                json!([
                    { "email": primary_unverified, "primary": true, "verified": false },
                    { "email": secondary, "primary": false, "verified": true },
                ]),
            ),
        )
        .await;
    assert_eq!(landing.refusal().as_deref(), Some("email_unverified"));

    // Verified but not primary: refused.
    let verified_secondary = unique_email("gh-verified-not-primary");
    let landing = h
        .sign_in(
            "github",
            Person::github(
                &format!("v-{}", &uid()[..8]),
                Some(&held),
                json!([
                    { "email": unique_email("gh-unverified-primary"), "primary": true, "verified": false },
                    { "email": verified_secondary, "primary": false, "verified": true },
                ]),
            ),
        )
        .await;
    assert_eq!(landing.refusal().as_deref(), Some("email_unverified"));

    for nobody in [&primary_unverified, &secondary, &verified_secondary] {
        assert!(h.user(nobody).await.is_none(), "{nobody}");
    }
    assert!(
        h.user_by_id(holder).await.oauth_providers.is_empty(),
        "the profile address linked nothing"
    );
    h.finish().await;
}

#[tokio::test]
async fn an_account_that_proved_the_address_is_linked_and_found_by_identity_after() {
    let h = Harness::start(|_| {}).await;
    let email = unique_email("linked");
    let existing = h.password_account(&email, Proof::EmailedLink).await;

    let person = Person::google(&email, true);
    let landing = h.sign_in("google", person.clone()).await;
    assert_eq!(landing.location, signed_in_url(), "{landing:?}");
    let me = h.me(&landing.session().unwrap()).await;
    assert_eq!(me["id"], existing.to_hex().as_str(), "the same account");
    let user = h.user_by_id(existing).await;
    assert_eq!(user.oauth_providers, vec![person.identity()]);
    assert!(user.password_hash.is_some(), "the password still works");
    assert_eq!(h.count(&email).await, 1);

    // Returning, after the address at Google changed: found by the identity,
    // not by address, and linked once.
    let moved = unique_email("moved");
    let Person::Google { sub, .. } = person else {
        unreachable!()
    };
    let returning = Person::Google {
        sub,
        email: moved.clone(),
        verified: true,
    };
    let landing = h.sign_in("google", returning).await;
    let me = h.me(&landing.session().expect("signed in")).await;
    assert_eq!(me["id"], existing.to_hex().as_str());
    assert_eq!(h.user_by_id(existing).await.oauth_providers.len(), 1);
    assert!(h.user(&moved).await.is_none());

    // An address the account holds in another letter case is the same
    // address.
    let mixed = format!("Mixed.Case-{}@Example.COM", &uid()[..8]);
    let mixed_account = h.password_account(&mixed, Proof::EmailedLink).await;
    let landing = h
        .sign_in("google", Person::google(&mixed.to_lowercase(), true))
        .await;
    let me = h.me(&landing.session().expect("signed in")).await;
    assert_eq!(me["id"], mixed_account.to_hex().as_str());
    assert!(
        h.user(&mixed.to_lowercase()).await.is_none(),
        "no second account"
    );
    h.finish().await;
}

#[tokio::test]
async fn an_account_that_did_not_prove_the_address_is_never_linked() {
    // No email service: activation is not by emailed link.
    let h = Harness::start(|_| {}).await;
    for proof in [Proof::Never, Proof::NoEmailService, Proof::Legacy] {
        let email = unique_email("unproven");
        let id = h.password_account(&email, proof).await;
        for person in [
            Person::google(&email, true),
            Person::github(
                &format!("gh-{}", &uid()[..8]),
                None,
                json!([{ "email": email, "primary": true, "verified": true }]),
            ),
        ] {
            let provider = match person {
                Person::Google { .. } => "google",
                Person::GitHub { .. } => "github",
            };
            let landing = h.sign_in(provider, person).await;
            assert_eq!(
                landing.refusal().as_deref(),
                Some("account_exists"),
                "{proof:?} via {provider}"
            );
            assert!(landing.session().is_none());
        }
        assert!(
            h.user_by_id(id).await.oauth_providers.is_empty(),
            "{proof:?}: nothing linked"
        );
        assert_eq!(h.count(&email).await, 1);
    }
    h.finish().await;

    // Where activation IS by emailed link, an activated account from before
    // `email_proven` existed did prove its address.
    let h = Harness::start(|s| s.email.api_key = "fake-email-service-key".to_string()).await;
    let email = unique_email("legacy");
    let id = h.password_account(&email, Proof::Legacy).await;
    let landing = h.sign_in("google", Person::google(&email, true)).await;
    let me = h.me(&landing.session().expect("linked")).await;
    assert_eq!(me["id"], id.to_hex().as_str());
    let user = h.user_by_id(id).await;
    assert_eq!(user.oauth_providers.len(), 1);
    assert_eq!(user.email_proven, Some(true));
    h.finish().await;
}

#[tokio::test]
async fn the_providers_list_shows_what_works_and_other_names_are_404() {
    let h = Harness::start(|_| {}).await;
    assert_eq!(h.providers().await, json!(["google", "github"]));
    for retired in ["facebook", "linkedin", "microsoft"] {
        for path in [
            format!("/api/oauth/{retired}"),
            format!("/api/oauth/{retired}/callback?code=x&state=y"),
        ] {
            let response = h.get(&path, None).await;
            assert_eq!(response.status(), 404, "{path}");
            let body: Value = response.json().await.unwrap();
            let message = body["message"].as_str().unwrap();
            assert!(message.contains("not supported"), "{path}: {message}");
        }
    }
    for unknown in [
        "/api/oauth/myspace",
        "/api/oauth/Google",
        "/api/oauth/myspace/callback",
    ] {
        assert_eq!(h.get(unknown, None).await.status(), 404, "{unknown}");
    }
    h.finish().await;

    // Only GitHub configured: Google has no button and no route.
    let h = Harness::start(|s| s.oauth.google = None).await;
    assert_eq!(h.providers().await, json!(["github"]));
    let response = h.get("/api/oauth/google", None).await;
    assert_eq!(response.status(), 404);
    let body: Value = response.json().await.unwrap();
    assert!(
        body["message"].as_str().unwrap().contains("not configured"),
        "{body}"
    );
    h.finish().await;

    // OAuth off: no buttons, no routes.
    let h = Harness::start(|s| s.oauth.enabled = false).await;
    assert_eq!(h.providers().await, json!([]));
    assert_eq!(h.get("/api/oauth/google", None).await.status(), 404);
    assert_eq!(
        h.get("/api/oauth/github/callback?code=x&state=y", None)
            .await
            .status(),
        404
    );
    h.finish().await;
}
