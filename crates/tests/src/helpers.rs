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
        }
        data
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
