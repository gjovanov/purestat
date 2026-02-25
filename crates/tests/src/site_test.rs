use crate::helpers::TestClient;

#[tokio::test]
async fn test_site_crud() {
    let mut client = TestClient::new();
    let uid = uuid::Uuid::new_v4().to_string();
    client
        .register(
            &format!("site-{uid}@purestat.test"),
            &format!("site-{}", &uid[..8]),
            "TestPass123!",
        )
        .await;

    // Create org first
    let org: serde_json::Value = client
        .post(
            "/api/org",
            &serde_json::json!({
                "name": "Site Test Org",
                "slug": format!("site-org-{}", &uid[..8])
            }),
        )
        .await
        .json()
        .await
        .unwrap();
    let org_id = org["id"].as_str().unwrap();

    // Create site
    let resp = client
        .post(
            &format!("/api/org/{org_id}/site"),
            &serde_json::json!({
                "domain": format!("{}.example.com", &uid[..8]),
                "name": "Test Site"
            }),
        )
        .await;
    assert_eq!(resp.status(), 201);
    let site: serde_json::Value = resp.json().await.unwrap();
    let site_id = site["id"].as_str().unwrap();

    // List sites
    let resp = client.get(&format!("/api/org/{org_id}/site")).await;
    assert_eq!(resp.status(), 200);

    // Update site
    let resp = client
        .put(
            &format!("/api/org/{org_id}/site/{site_id}"),
            &serde_json::json!({ "name": "Updated Site" }),
        )
        .await;
    assert_eq!(resp.status(), 200);

    // Delete site
    let resp = client
        .delete(&format!("/api/org/{org_id}/site/{site_id}"))
        .await;
    assert_eq!(resp.status(), 204);
}
