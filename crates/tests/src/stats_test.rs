use crate::helpers::{page_rows, user_with_site, TestClient};

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

/// GHSA-7f5h-5qwr-rxh5: membership of ONE org is not access to another org's
/// site. A member of org A who puts org B's site id in org A's path gets 404,
/// from stats, realtime and export alike: neither data nor existence leaks.
#[tokio::test]
async fn test_a_site_from_another_org_is_not_found() {
    let (_owner, _org_b, site_b, _domain_b) = user_with_site("victim").await;
    let (intruder, org_a, _site_a, _domain_a) = user_with_site("intruder").await;

    let stats = intruder
        .post(
            &format!("/api/org/{org_a}/site/{site_b}/stats"),
            &serde_json::json!({ "date_range": "7d", "metrics": ["visitors", "pageviews"] }),
        )
        .await;
    assert_eq!(stats.status(), 404, "stats for another org's site");

    let realtime = intruder.get(&format!("/api/org/{org_a}/site/{site_b}/realtime")).await;
    assert_eq!(realtime.status(), 404, "realtime for another org's site");

    let export = intruder
        .get(&format!("/api/org/{org_a}/site/{site_b}/export?date_from=2026-01-01&date_to=2026-12-31"))
        .await;
    assert_eq!(export.status(), 404, "export of another org's site");
}

/// GHSA-7f5h-5qwr-rxh5: what a caller sends is data, never SQL. Another
/// tenant's site has a real pageview; a filter value that tries to widen the
/// WHERE clause must match nothing on the caller's own (empty) site, and a
/// date that is not a date is refused with 400 before any query runs.
#[tokio::test]
async fn test_filter_values_and_dates_are_data_not_sql() {
    let (victim, victim_org, victim_site, victim_domain) = user_with_site("victim").await;
    let (client, org_id, site_id, _domain) = user_with_site("owner").await;

    // One real pageview on the VICTIM's site, through the public ingest path.
    let sent = client
        .post(
            "/api/event",
            &serde_json::json!({
                "domain": victim_domain,
                "name": "pageview",
                "url": format!("https://{victim_domain}/private-report"),
                "referrer": "",
                "screen_width": 1280
            }),
        )
        .await;
    assert!(sent.status().is_success(), "ingest: {}", sent.status());
    let mut landed = false;
    for _ in 0..30 {
        if page_rows(&victim, &victim_org, &victim_site, serde_json::json!([])).await.1 > 0 {
            landed = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    assert!(landed, "the victim's pageview never became queryable");

    // The caller's own site has no data. An injected WHERE would pull the
    // victim's row in; a bound value matches nothing.
    let (status, rows) = page_rows(
        &client,
        &org_id,
        &site_id,
        serde_json::json!([{ "dimension": "page", "operator": "is", "value": "x' OR site_id != 0 OR '1'='1" }]),
    )
    .await;
    assert_eq!(status, 200, "a quoted filter value is a value");
    assert_eq!(rows, 0, "the filter value widened the query into another tenant's data");

    let bad_date = client
        .post(
            &format!("/api/org/{org_id}/site/{site_id}/stats"),
            &serde_json::json!({ "date_from": "2026-01-01' OR '1'='1", "date_to": "2026-12-31", "metrics": ["visitors"] }),
        )
        .await;
    assert_eq!(bad_date.status(), 400, "a date that is not a date");

    let bad_export = client
        .get(&format!("/api/org/{org_id}/site/{site_id}/export?date_from=2026-01-01%27%20OR%201%3D1%20--&date_to=2026-12-31"))
        .await;
    assert_eq!(bad_export.status(), 400, "an export date that is not a date");
}

/// An empty dimension list is a bad request. It used to index `dimensions[0]`
/// and panic, which dropped the connection instead of answering.
#[tokio::test]
async fn test_empty_dimensions_is_a_bad_request() {
    let (client, org_id, site_id, _domain) = user_with_site("owner").await;
    let resp = client
        .post(
            &format!("/api/org/{org_id}/site/{site_id}/stats"),
            &serde_json::json!({ "date_range": "7d", "metrics": ["visitors"], "dimensions": [] }),
        )
        .await;
    assert_eq!(resp.status(), 400, "an empty dimension list");
}
