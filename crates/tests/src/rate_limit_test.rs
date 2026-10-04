use crate::helpers::base_url;
use reqwest::{Client, Response, StatusCode};
use serde_json::{Value, json};

/// A documentation-range IPv6 /64 of its own, so a rerun inside the 60-second
/// window never lands in a bucket an earlier run filled.
fn fresh_client_prefix() -> String {
    let id = uuid::Uuid::new_v4();
    let b = id.as_bytes();
    format!(
        "2001:db8:{:x}:{:x}",
        u16::from_be_bytes([b[0], b[1]]),
        u16::from_be_bytes([b[2], b[3]])
    )
}

/// The request as it reaches the API in production: the front proxy appended
/// the address it saw to whatever X-Forwarded-For the client sent.
async fn resend_activation(client: &Client, forwarded_for: &str) -> Response {
    client
        .post(format!("{}/api/auth/resend-activation", base_url()))
        .header("x-forwarded-for", forwarded_for)
        .json(&json!({ "email": "nobody@example.invalid" }))
        .send()
        .await
        .expect("Request failed")
}

fn header<T: std::str::FromStr>(response: &Response, name: &str) -> T {
    response
        .headers()
        .get(name)
        .unwrap_or_else(|| panic!("the response carries {name}"))
        .to_str()
        .unwrap()
        .parse()
        .unwrap_or_else(|_| panic!("{name} is a number"))
}

#[tokio::test]
async fn test_auth_requests_past_the_limit_get_429_per_client_address() {
    let client = Client::new();
    let prefix = fresh_client_prefix();
    let addr = format!("{prefix}::7");

    let first = resend_activation(&client, &format!("1.1.1.1, {addr}")).await;
    assert_eq!(first.status(), StatusCode::OK);
    let limit: u32 = header(&first, "x-ratelimit-limit");
    assert_eq!(header::<u32>(&first, "x-ratelimit-remaining"), limit - 1);

    for n in 2..=limit {
        let r = resend_activation(&client, &format!("1.1.1.1, {addr}")).await;
        assert_eq!(r.status(), StatusCode::OK, "request {n} of {limit}");
    }

    // One more is refused. A different client-written entry on the left does
    // not make it a different client.
    let refused = resend_activation(&client, &format!("9.9.9.9, {addr}")).await;
    assert_eq!(refused.status(), StatusCode::TOO_MANY_REQUESTS);
    let retry_after: u64 = header(&refused, "retry-after");
    assert!(
        (1..=60).contains(&retry_after),
        "Retry-After: {retry_after}"
    );
    assert_eq!(header::<u32>(&refused, "x-ratelimit-remaining"), 0);
    let body: Value = refused.json().await.unwrap();
    assert_eq!(body["error"], "rate_limited");

    // Another address in the same /64 is the same IPv6 client.
    let same_net = resend_activation(&client, &format!("{prefix}::8")).await;
    assert_eq!(same_net.status(), StatusCode::TOO_MANY_REQUESTS);

    // Another client is not affected.
    let other = resend_activation(&client, &format!("{}::7", fresh_client_prefix())).await;
    assert_eq!(other.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_health_is_not_rate_limited() {
    let r = Client::new()
        .get(format!("{}/api/health", base_url()))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(r.status(), StatusCode::OK);
    assert!(r.headers().get("x-ratelimit-limit").is_none());
}
