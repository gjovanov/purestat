use crate::helpers::TestClient;

#[tokio::test]
async fn test_goal_crud() {
    let mut client = TestClient::new();
    let uid = uuid::Uuid::new_v4().to_string();
    client
        .register(
            &format!("goal-{uid}@purestat.test"),
            &format!("goal-{}", &uid[..8]),
            "TestPass123!",
        )
        .await;

    let org: serde_json::Value = client
        .post(
            "/api/org",
            &serde_json::json!({
                "name": "Goal Test Org",
                "slug": format!("goal-org-{}", &uid[..8])
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
                "domain": format!("goal-{}.example.com", &uid[..8]),
                "name": "Goal Test Site"
            }),
        )
        .await
        .json()
        .await
        .unwrap();
    let site_id = site["id"].as_str().unwrap();

    // Create goal
    let resp = client
        .post(
            &format!("/api/org/{org_id}/site/{site_id}/goal"),
            &serde_json::json!({
                "goal_type": "custom_event",
                "name": "Signup",
                "event_name": "signup"
            }),
        )
        .await;
    assert_eq!(resp.status(), 201);
    let goal: serde_json::Value = resp.json().await.unwrap();
    let goal_id = goal["id"].as_str().unwrap();

    // List goals
    let resp = client
        .get(&format!("/api/org/{org_id}/site/{site_id}/goal"))
        .await;
    assert_eq!(resp.status(), 200);

    // Delete goal
    let resp = client
        .delete(&format!("/api/org/{org_id}/site/{site_id}/goal/{goal_id}"))
        .await;
    assert_eq!(resp.status(), 204);
}
