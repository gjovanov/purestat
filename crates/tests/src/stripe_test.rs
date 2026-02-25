use crate::helpers::TestClient;

#[tokio::test]
async fn test_plans_endpoint() {
    let client = TestClient::new();
    let resp = client.get("/api/stripe/plans").await;
    assert_eq!(resp.status(), 200);
    let data: serde_json::Value = resp.json().await.unwrap();
    let plans = data["plans"].as_array().unwrap();
    assert_eq!(plans.len(), 3);
    assert_eq!(plans[0]["name"].as_str().unwrap(), "Free");
    assert_eq!(plans[1]["name"].as_str().unwrap(), "Pro");
    assert_eq!(plans[2]["name"].as_str().unwrap(), "Business");
}
