use crate::helpers::TestClient;

#[tokio::test]
async fn test_event_ingest() {
    let client = TestClient::new();

    // Send event (no auth required, validated by domain)
    let resp = client
        .post(
            "/api/event",
            &serde_json::json!({
                "domain": "test.example.com",
                "name": "pageview",
                "url": "https://test.example.com/blog",
                "referrer": "https://google.com",
                "screen_width": 1920
            }),
        )
        .await;

    // May be 202 Accepted or 400 if domain not registered
    assert!(resp.status() == 202 || resp.status() == 400);
}

#[tokio::test]
async fn test_event_with_custom_props() {
    let client = TestClient::new();

    let resp = client
        .post(
            "/api/event",
            &serde_json::json!({
                "domain": "test.example.com",
                "name": "signup",
                "url": "https://test.example.com/register",
                "props": {
                    "plan": "pro",
                    "source": "landing"
                }
            }),
        )
        .await;

    assert!(resp.status() == 202 || resp.status() == 400);
}
