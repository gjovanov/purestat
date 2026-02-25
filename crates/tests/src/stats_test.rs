use crate::helpers::TestClient;

#[tokio::test]
async fn test_stats_query() {
    let mut client = TestClient::new();
    let uid = uuid::Uuid::new_v4().to_string();
    client
        .register(
            &format!("stats-{uid}@purestat.test"),
            &format!("stats-{}", &uid[..8]),
            "TestPass123!",
        )
        .await;

    let org: serde_json::Value = client
        .post(
            "/api/org",
            &serde_json::json!({
                "name": "Stats Test Org",
                "slug": format!("stats-org-{}", &uid[..8])
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
                "domain": format!("stats-{}.example.com", &uid[..8]),
                "name": "Stats Test Site"
            }),
        )
        .await
        .json()
        .await
        .unwrap();
    let site_id = site["id"].as_str().unwrap();

    // Query stats (may return empty data)
    let resp = client
        .post(
            &format!("/api/org/{org_id}/site/{site_id}/stats"),
            &serde_json::json!({
                "date_range": "7d",
                "metrics": ["visitors", "pageviews"]
            }),
        )
        .await;
    assert_eq!(resp.status(), 200);
}
