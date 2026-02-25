use crate::helpers::TestClient;

#[tokio::test]
async fn test_invite_flow() {
    let mut client = TestClient::new();
    let uid = uuid::Uuid::new_v4().to_string();
    client
        .register(
            &format!("inv-{uid}@purestat.test"),
            &format!("inv-{}", &uid[..8]),
            "TestPass123!",
        )
        .await;

    let org: serde_json::Value = client
        .post(
            "/api/org",
            &serde_json::json!({
                "name": "Invite Test Org",
                "slug": format!("inv-org-{}", &uid[..8])
            }),
        )
        .await
        .json()
        .await
        .unwrap();
    let org_id = org["id"].as_str().unwrap();

    // Create invite
    let resp = client
        .post(
            &format!("/api/org/{org_id}/invite"),
            &serde_json::json!({
                "role": "viewer",
                "max_uses": 1,
                "expires_in_hours": 24
            }),
        )
        .await;
    assert_eq!(resp.status(), 201);
    let invite: serde_json::Value = resp.json().await.unwrap();
    let code = invite["code"].as_str().unwrap();

    // Get invite info (public)
    let public_client = TestClient::new();
    let resp = public_client.get(&format!("/api/invite/{code}")).await;
    assert_eq!(resp.status(), 200);

    // Accept invite as another user
    let mut client2 = TestClient::new();
    client2
        .register(
            &format!("inv2-{uid}@purestat.test"),
            &format!("inv2-{}", &uid[..8]),
            "TestPass123!",
        )
        .await;
    let resp = client2
        .post(&format!("/api/invite/{code}/accept"), &serde_json::json!({}))
        .await;
    assert_eq!(resp.status(), 200);

    // List invites
    let resp = client.get(&format!("/api/org/{org_id}/invite")).await;
    assert_eq!(resp.status(), 200);
}
