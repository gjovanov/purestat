use crate::helpers::TestClient;

#[tokio::test]
async fn test_api_key_crud() {
    let mut client = TestClient::new();
    let uid = uuid::Uuid::new_v4().to_string();
    client
        .register(
            &format!("key-{uid}@purestat.test"),
            &format!("key-{}", &uid[..8]),
            "TestPass123!",
        )
        .await;

    let org: serde_json::Value = client
        .post(
            "/api/org",
            &serde_json::json!({
                "name": "API Key Test Org",
                "slug": format!("key-org-{}", &uid[..8])
            }),
        )
        .await
        .json()
        .await
        .unwrap();
    let org_id = org["id"].as_str().unwrap();

    let site: serde_json::Value = client
        .post(
            &format!("/api/org/{org_id}/site"),
            &serde_json::json!({
                "domain": format!("key-{}.example.com", &uid[..8]),
                "name": "API Key Test Site"
            }),
        )
        .await
        .json()
        .await
        .unwrap();
    let site_id = site["id"].as_str().unwrap();

    // Create API key
    let resp = client
        .post(
            &format!("/api/org/{org_id}/site/{site_id}/api-key"),
            &serde_json::json!({
                "name": "Test Key",
                "scopes": ["stats:read"]
            }),
        )
        .await;
    assert_eq!(resp.status(), 201);
    let key: serde_json::Value = resp.json().await.unwrap();
    assert!(key["key"].as_str().unwrap().starts_with("ps_"));
    let key_id = key["id"].as_str().unwrap();

    // List API keys
    let resp = client
        .get(&format!("/api/org/{org_id}/site/{site_id}/api-key"))
        .await;
    assert_eq!(resp.status(), 200);

    // Revoke API key
    let resp = client
        .delete(&format!(
            "/api/org/{org_id}/site/{site_id}/api-key/{key_id}"
        ))
        .await;
    assert_eq!(resp.status(), 204);
}
