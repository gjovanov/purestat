use crate::helpers::TestClient;

#[tokio::test]
async fn test_member_management() {
    let mut client = TestClient::new();
    let uid = uuid::Uuid::new_v4().to_string();
    client
        .register(
            &format!("mem-{uid}@purestat.test"),
            &format!("mem-{}", &uid[..8]),
            "TestPass123!",
        )
        .await;

    let org: serde_json::Value = client
        .post(
            "/api/org",
            &serde_json::json!({
                "name": "Member Test Org",
                "slug": format!("mem-org-{}", &uid[..8])
            }),
        )
        .await
        .json()
        .await
        .unwrap();
    let org_id = org["id"].as_str().unwrap();

    // List members (should have owner)
    let resp = client.get(&format!("/api/org/{org_id}/member")).await;
    assert_eq!(resp.status(), 200);
    let members: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert_eq!(members.len(), 1);
    assert_eq!(members[0]["role"].as_str().unwrap(), "owner");
}
