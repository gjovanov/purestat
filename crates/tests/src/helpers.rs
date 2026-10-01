use reqwest::Client;
use serde_json::Value;
use std::sync::OnceLock;

static BASE_URL: OnceLock<String> = OnceLock::new();

pub fn base_url() -> &'static str {
    BASE_URL.get_or_init(|| {
        std::env::var("API_URL").unwrap_or_else(|_| "http://localhost:3000".to_string())
    })
}

pub struct TestClient {
    client: Client,
    pub access_token: Option<String>,
}

impl TestClient {
    pub fn new() -> Self {
        Self {
            client: Client::builder().cookie_store(true).build().unwrap(),
            access_token: None,
        }
    }

    pub fn authenticated(token: &str) -> Self {
        Self {
            client: Client::builder().cookie_store(true).build().unwrap(),
            access_token: Some(token.to_string()),
        }
    }

    pub async fn post(&self, path: &str, body: &Value) -> reqwest::Response {
        let url = format!("{}{}", base_url(), path);
        let mut req = self.client.post(&url).json(body);
        if let Some(token) = &self.access_token {
            req = req.bearer_auth(token);
        }
        req.send().await.expect("Request failed")
    }

    pub async fn get(&self, path: &str) -> reqwest::Response {
        let url = format!("{}{}", base_url(), path);
        let mut req = self.client.get(&url);
        if let Some(token) = &self.access_token {
            req = req.bearer_auth(token);
        }
        req.send().await.expect("Request failed")
    }

    pub async fn put(&self, path: &str, body: &Value) -> reqwest::Response {
        let url = format!("{}{}", base_url(), path);
        let mut req = self.client.put(&url).json(body);
        if let Some(token) = &self.access_token {
            req = req.bearer_auth(token);
        }
        req.send().await.expect("Request failed")
    }

    pub async fn delete(&self, path: &str) -> reqwest::Response {
        let url = format!("{}{}", base_url(), path);
        let mut req = self.client.delete(&url);
        if let Some(token) = &self.access_token {
            req = req.bearer_auth(token);
        }
        req.send().await.expect("Request failed")
    }

    pub async fn register(
        &mut self,
        email: &str,
        username: &str,
        password: &str,
    ) -> Value {
        let body = serde_json::json!({
            "email": email,
            "username": username,
            "password": password,
            "display_name": username
        });
        let resp = self.post("/api/auth/register", &body).await;
        let data: Value = resp.json().await.unwrap();
        if let Some(token) = data["access_token"].as_str() {
            self.access_token = Some(token.to_string());
        } else {
            // Registration answers with a message. With an email service the
            // account waits for its activation link; without one, the API
            // auto-verifies it. The helper handles both, then signs in.
            self.activate_and_login(email, password).await;
        }
        data
    }

    /// Activate a new account and sign in, as a person clicking the emailed
    /// link would. A test cannot read an inbox, so it reads the activation
    /// code where the API stored it (Mongo, via the same `PURESTAT__*`
    /// settings the API runs with).
    async fn activate_and_login(&mut self, email: &str, password: &str) {
        let settings = purestat_config::Settings::load().expect("PURESTAT__* settings");
        let db = purestat_db::connect(&settings).await.expect("mongo");
        let user = db
            .collection::<bson::Document>("users")
            .find_one(bson::doc! { "email": email })
            .await
            .expect("find user")
            .expect("the user was just registered");
        let user_id = user.get_object_id("_id").expect("user _id");
        if user.get_bool("is_verified").unwrap_or(false) {
            // Auto-verified: no email service configured.
            self.login(email, password).await;
            return;
        }
        let code = db
            .collection::<bson::Document>("activation_codes")
            .find_one(bson::doc! { "user_id": user_id })
            .await
            .expect("find activation code")
            .expect("registration stored an activation code");
        let token = code.get_str("token").expect("token").to_string();
        let resp = self
            .post(
                "/api/auth/activate",
                &serde_json::json!({ "user_id": user_id.to_hex(), "token": token }),
            )
            .await;
        assert!(resp.status().is_success(), "activation failed: {}", resp.status());
        self.login(email, password).await;
    }

    pub async fn login(&mut self, email: &str, password: &str) -> Value {
        let body = serde_json::json!({
            "email": email,
            "password": password,
        });
        let resp = self.post("/api/auth/login", &body).await;
        let data: Value = resp.json().await.unwrap();
        if let Some(token) = data["access_token"].as_str() {
            self.access_token = Some(token.to_string());
        }
        data
    }
}

/// A signed-up user with an org of their own, and one site in it.
/// Returns the client, the org id, the site id and the site's domain.
pub async fn user_with_site(tag: &str) -> (TestClient, String, String, String) {
    let mut client = TestClient::new();
    let uid = uuid::Uuid::new_v4().to_string();
    client
        .register(
            &format!("{tag}-{uid}@purestat.test"),
            &format!("{tag}-{}", &uid[..8]),
            "TestPass123!",
        )
        .await;
    let org: serde_json::Value = client
        .post(
            "/api/org",
            &serde_json::json!({ "name": format!("{tag} org"), "slug": format!("{tag}-org-{}", &uid[..8]) }),
        )
        .await
        .json()
        .await
        .unwrap();
    let org_id = org["id"].as_str().unwrap().to_string();
    let domain = format!("{tag}-{}.example.com", &uid[..8]);
    let site: serde_json::Value = client
        .post(
            &format!("/api/org/{org_id}/site"),
            &serde_json::json!({ "domain": domain, "name": format!("{tag} site") }),
        )
        .await
        .json()
        .await
        .unwrap();
    let site_id = site["id"].as_str().unwrap().to_string();
    (client, org_id, site_id, domain)
}

/// Pageviews on the site's own stats page breakdown (0 until ingest flushes).
pub async fn page_rows(client: &TestClient, org_id: &str, site_id: &str, filters: serde_json::Value) -> (u16, usize) {
    let resp = client
        .post(
            &format!("/api/org/{org_id}/site/{site_id}/stats"),
            &serde_json::json!({ "date_range": "7d", "metrics": ["visitors"], "dimensions": ["page"], "filters": filters }),
        )
        .await;
    let status = resp.status().as_u16();
    let body: serde_json::Value = resp.json().await.unwrap_or_default();
    (status, body["dimensions"].as_array().map(|a| a.len()).unwrap_or(0))
}
