use crate::helpers::TestClient;

#[tokio::test]
async fn test_multi_site_stats_query() {
    let mut client = TestClient::new();
    let uid = uuid::Uuid::new_v4().to_string();
    client
        .register(
            &format!("ms-{uid}@purestat.test"),
            &format!("ms-{}", &uid[..8]),
            "TestPass123!",
        )
        .await;

    // Create org
    let org: serde_json::Value = client
        .post(
            "/api/org",
            &serde_json::json!({
                "name": "Multi Site Test Org",
                "slug": format!("ms-org-{}", &uid[..8])
            }),
        )
        .await
        .json()
        .await
        .unwrap();
    let org_id = org["id"].as_str().unwrap();

    // Create one site (free plan allows 1)
    let site1: serde_json::Value = client
        .post(
            &format!("/api/org/{org_id}/site"),
            &serde_json::json!({
                "domain": format!("ms1-{}.example.com", &uid[..8]),
                "name": "Multi Site 1"
            }),
        )
        .await
        .json()
        .await
        .unwrap();
    let site1_id = site1["id"].as_str().unwrap();

    // Test multi-site stats endpoint with single site (validates endpoint works)
    let resp = client
        .post(
            &format!("/api/org/{org_id}/analytics/stats"),
            &serde_json::json!({
                "site_ids": [site1_id],
                "date_range": "7d",
                "metrics": ["visitors", "pageviews"]
            }),
        )
        .await;
    assert_eq!(resp.status(), 200);
    let data: serde_json::Value = resp.json().await.unwrap();
    assert!(data["metrics"].is_object());

    // Test with dimensions
    let resp = client
        .post(
            &format!("/api/org/{org_id}/analytics/stats"),
            &serde_json::json!({
                "site_ids": [site1_id],
                "date_range": "7d",
                "metrics": ["visitors"],
                "dimensions": ["country"],
                "limit": 5
            }),
        )
        .await;
    assert_eq!(resp.status(), 200);

    // Test with timeseries
    let resp = client
        .post(
            &format!("/api/org/{org_id}/analytics/stats"),
            &serde_json::json!({
                "site_ids": [site1_id],
                "date_range": "7d",
                "metrics": ["visitors", "pageviews"],
                "interval": "day"
            }),
        )
        .await;
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_multi_site_realtime() {
    let mut client = TestClient::new();
    let uid = uuid::Uuid::new_v4().to_string();
    client
        .register(
            &format!("msrt-{uid}@purestat.test"),
            &format!("msrt-{}", &uid[..8]),
            "TestPass123!",
        )
        .await;

    let org: serde_json::Value = client
        .post(
            "/api/org",
            &serde_json::json!({
                "name": "RT Test Org",
                "slug": format!("rt-org-{}", &uid[..8])
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
                "domain": format!("rt-{}.example.com", &uid[..8]),
                "name": "RT Test Site"
            }),
        )
        .await
        .json()
        .await
        .unwrap();
    let site_id = site["id"].as_str().unwrap();

    // Test multi-site realtime endpoint
    let resp = client
        .get(&format!(
            "/api/org/{org_id}/analytics/realtime?site_ids={site_id}"
        ))
        .await;
    assert_eq!(resp.status(), 200);
    let data: serde_json::Value = resp.json().await.unwrap();
    assert!(data["current_visitors"].is_number());
    assert!(data["visitors"].is_array());
    assert!(data["top_pages"].is_array());
}

#[tokio::test]
async fn test_multi_site_invalid_site() {
    let mut client = TestClient::new();
    let uid = uuid::Uuid::new_v4().to_string();
    client
        .register(
            &format!("msinv-{uid}@purestat.test"),
            &format!("msinv-{}", &uid[..8]),
            "TestPass123!",
        )
        .await;

    let org: serde_json::Value = client
        .post(
            "/api/org",
            &serde_json::json!({
                "name": "Invalid Site Test Org",
                "slug": format!("inv-org-{}", &uid[..8])
            }),
        )
        .await
        .json()
        .await
        .unwrap();
    let org_id = org["id"].as_str().unwrap();

    // Test with site_id not belonging to org -> 400
    let resp = client
        .post(
            &format!("/api/org/{org_id}/analytics/stats"),
            &serde_json::json!({
                "site_ids": ["000000000000000000000000"],
                "date_range": "7d",
                "metrics": ["visitors"]
            }),
        )
        .await;
    assert_eq!(resp.status(), 400);

    // Test realtime with invalid site -> 400
    let resp = client
        .get(&format!(
            "/api/org/{org_id}/analytics/realtime?site_ids=000000000000000000000000"
        ))
        .await;
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_multi_site_empty_ids() {
    let mut client = TestClient::new();
    let uid = uuid::Uuid::new_v4().to_string();
    client
        .register(
            &format!("msempty-{uid}@purestat.test"),
            &format!("msempty-{}", &uid[..8]),
            "TestPass123!",
        )
        .await;

    let org: serde_json::Value = client
        .post(
            "/api/org",
            &serde_json::json!({
                "name": "Empty IDs Test Org",
                "slug": format!("empty-org-{}", &uid[..8])
            }),
        )
        .await
        .json()
        .await
        .unwrap();
    let org_id = org["id"].as_str().unwrap();

    // Test with empty site_ids -> 400
    let resp = client
        .post(
            &format!("/api/org/{org_id}/analytics/stats"),
            &serde_json::json!({
                "site_ids": [],
                "date_range": "7d",
                "metrics": ["visitors"]
            }),
        )
        .await;
    assert_eq!(resp.status(), 400);
}

// --- Security (GHSA-7f5h-5qwr-rxh5), for the multi-site routes ---------------

use crate::helpers::{page_rows, user_with_site};

/// Another org's site id is refused, alone or mixed in with the caller's own
/// sites, on both multi-site routes; nothing of the victim's comes back.
#[tokio::test]
async fn test_multi_site_refuses_another_orgs_site() {
    let (_victim, _victim_org, victim_site, _) = user_with_site("victim").await;
    let (intruder, intruder_org, intruder_site, _) = user_with_site("intruder").await;
    let stats = format!("/api/org/{intruder_org}/analytics/stats");
    for site_ids in [vec![victim_site.clone()], vec![intruder_site.clone(), victim_site.clone()]] {
        let resp = intruder
            .post(&stats, &serde_json::json!({ "site_ids": site_ids, "date_range": "7d", "metrics": ["visitors"] }))
            .await;
        assert_eq!(resp.status(), 400, "multi-site stats with another org's site {site_ids:?}");
    }
    let realtime = format!("/api/org/{intruder_org}/analytics/realtime?site_ids={intruder_site},{victim_site}");
    assert_eq!(intruder.get(&realtime).await.status(), 400, "multi-site realtime with another org's site");
}

/// What a caller sends to the multi-site stats route is data, never SQL: a
/// victim has a real pageview, and a filter value that tries to widen the
/// WHERE clause matches nothing on the caller's own sites. A date that is not
/// a date, and an empty dimension list, are refused with 400.
#[tokio::test]
async fn test_multi_site_filter_values_and_dates_are_data_not_sql() {
    let (victim, victim_org, victim_site, victim_domain) = user_with_site("victim").await;
    let (client, org_id, site_id, _) = user_with_site("owner").await;

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

    let stats = format!("/api/org/{org_id}/analytics/stats");
    let hostile = serde_json::json!({
        "site_ids": [site_id],
        "date_range": "7d",
        "metrics": ["visitors"],
        "dimensions": ["page"],
        "filters": [{ "dimension": "page", "operator": "is", "value": "x' OR site_id != 0 OR '1'='1" }]
    });
    let resp = client.post(&stats, &hostile).await;
    assert_eq!(resp.status(), 200, "a quoted filter value is a value");
    let body: serde_json::Value = resp.json().await.unwrap();
    let rows = body["dimensions"].as_array().map(|a| a.len()).unwrap_or(0);
    assert_eq!(rows, 0, "the filter value widened the query into another tenant's data: {body}");

    let bad_date = serde_json::json!({ "site_ids": [site_id], "date_from": "2026-01-01' OR '1'='1", "date_to": "2026-12-31", "metrics": ["visitors"] });
    assert_eq!(client.post(&stats, &bad_date).await.status(), 400, "a date that is not a date");

    let empty_dims = serde_json::json!({ "site_ids": [site_id], "date_range": "7d", "metrics": ["visitors"], "dimensions": [] });
    assert_eq!(client.post(&stats, &empty_dims).await.status(), 400, "an empty dimension list");
}
