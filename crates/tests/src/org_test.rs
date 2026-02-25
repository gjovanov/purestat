use crate::helpers::TestClient;

#[tokio::test]
async fn test_org_crud() {
    let mut client = TestClient::new();
    let uid = uuid::Uuid::new_v4().to_string();
    let email = format!("org-{uid}@purestat.test");
    let username = format!("org-{}", &uid[..8]);
    client.register(&email, &username, "TestPass123!").await;

    // Create org
    let resp = client
        .post(
            "/api/org",
            &serde_json::json!({
                "name": "Test Org",
                "slug": format!("test-org-{}", &uid[..8])
            }),
        )
        .await;
    assert_eq!(resp.status(), 201);
    let org: serde_json::Value = resp.json().await.unwrap();
    let org_id = org["id"].as_str().unwrap();

    // Get org
    let resp = client.get(&format!("/api/org/{org_id}")).await;
    assert_eq!(resp.status(), 200);

    // Update org
    let resp = client
        .put(
            &format!("/api/org/{org_id}"),
            &serde_json::json!({ "name": "Updated Org" }),
        )
        .await;
    assert_eq!(resp.status(), 200);

    // List orgs
    let resp = client.get("/api/org").await;
    assert_eq!(resp.status(), 200);
    let orgs: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert!(!orgs.is_empty());

    // Delete org
    let resp = client.delete(&format!("/api/org/{org_id}")).await;
    assert_eq!(resp.status(), 204);
}
