use crate::helpers::TestClient;

#[tokio::test]
async fn test_register_and_login() {
    let mut client = TestClient::new();
    let uid = uuid::Uuid::new_v4().to_string();
    let email = format!("test-{uid}@purestat.test");
    let username = format!("user-{}", &uid[..8]);

    // Register
    let data = client.register(&email, &username, "TestPass123!").await;
    assert!(data["user"]["id"].as_str().is_some());
    assert_eq!(data["user"]["email"].as_str().unwrap(), email);

    // Login
    let mut client2 = TestClient::new();
    let data2 = client2.login(&email, "TestPass123!").await;
    assert!(data2["access_token"].as_str().is_some());
}

#[tokio::test]
async fn test_me_endpoint() {
    let mut client = TestClient::new();
    let uid = uuid::Uuid::new_v4().to_string();
    let email = format!("me-{uid}@purestat.test");
    let username = format!("me-{}", &uid[..8]);

    client.register(&email, &username, "TestPass123!").await;

    let resp = client.get("/api/me").await;
    assert_eq!(resp.status(), 200);
    let data: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(data["email"].as_str().unwrap(), email);
}

#[tokio::test]
async fn test_login_invalid_credentials() {
    let mut client = TestClient::new();
    let data = client.login("nonexistent@test.com", "wrong").await;
    assert!(data["error"].as_str().is_some());
}
