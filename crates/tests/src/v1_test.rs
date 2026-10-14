//! PS1: `X-API-Key` and `POST /api/v1/query`, against the fixed fixture of
//! `trust_test::fixture` (its doc comment lists the traffic).

use serde_json::{Value, json};

use crate::seed::{self, v1};
use crate::trust_test::{DAY, Fixture, fixture};

async fn key(f: &Fixture) -> String {
    seed::api_key(&f.client, &f.org_id, &f.site_id, &["stats:read"])
        .await
        .0
}

fn day(extra: Value) -> Value {
    let mut body = json!({ "date_from": DAY, "date_to": DAY });
    for (k, v) in extra.as_object().unwrap() {
        body[k] = v.clone();
    }
    body
}

async fn ok(key: &str, body: Value) -> Value {
    let (status, answer) = v1(Some(key), &body).await;
    assert_eq!(status, 200, "{body} -> {answer}");
    answer
}

#[tokio::test]
async fn test_an_api_key_must_be_valid_unrevoked_and_scoped() {
    let f = fixture("apikey").await;
    let body = day(json!({ "metrics": ["visitors"] }));
    let unauthorized = (401, json!({ "error": "unauthorized" }));

    assert_eq!(v1(None, &body).await, unauthorized, "no key");
    assert_eq!(v1(Some(""), &body).await, unauthorized, "an empty key");
    assert_eq!(
        v1(Some("ps_not_a_key_at_all_000000000000000"), &body).await,
        unauthorized,
        "an unknown key"
    );

    let (wrong_scope, _) =
        seed::api_key(&f.client, &f.org_id, &f.site_id, &["realtime:read"]).await;
    assert_eq!(
        v1(Some(&wrong_scope), &body).await,
        unauthorized,
        "a key without stats:read"
    );

    let (key, key_id) = seed::api_key(&f.client, &f.org_id, &f.site_id, &["stats:read"]).await;
    let answer = ok(&key, body.clone()).await;
    assert_eq!(answer["rows"][0]["metrics"]["visitors"], 4, "{answer}");

    let db = purestat_db::connect(&seed::settings()).await.unwrap();
    let stored = db
        .collection::<bson::Document>("api_keys")
        .find_one(bson::doc! { "_id": bson::oid::ObjectId::parse_str(&key_id).unwrap() })
        .await
        .unwrap()
        .unwrap();
    assert!(
        stored.get_datetime("last_used_at").is_ok(),
        "a use is recorded: {stored}"
    );

    // A bad body is a 400 for a good key, and still a 401 without one.
    let (status, bad) = v1(Some(&key), &json!({ "metrics": ["visitors"] })).await;
    assert_eq!(status, 400, "{bad}");
    assert!(
        bad["error"].as_str().unwrap().contains("date_from"),
        "{bad}"
    );
    assert_eq!(v1(None, &json!({})).await, unauthorized);

    let revoked = f
        .client
        .delete(&format!(
            "/api/org/{}/site/{}/api-key/{key_id}",
            f.org_id, f.site_id
        ))
        .await;
    assert_eq!(revoked.status(), 204);
    assert_eq!(v1(Some(&key), &body).await, unauthorized, "a revoked key");
}

#[tokio::test]
async fn test_v1_totals_without_a_dimension() {
    let f = fixture("v1totals").await;
    let key = key(&f).await;
    let answer = ok(
        &key,
        day(json!({
            "metrics": ["visitors", "visits", "pageviews", "bounce_rate", "visit_duration", "reads", "median_read_s"],
            "goals": ["signup", "thanks"],
        })),
    )
    .await;
    assert_eq!(
        answer["rows"],
        json!([{
            "dimensions": {},
            "metrics": {
                "visitors": 4, "visits": 4, "pageviews": 6, "bounce_rate": 0.5,
                "visit_duration": 105.0, "reads": 3, "median_read_s": 40.0,
            },
            "goals": { "signup": 1, "thanks": 1 },
            "conversion_rates": { "signup": 0.25, "thanks": 0.25 },
        }]),
        "{answer}"
    );
    assert_eq!(
        answer["meta"],
        json!({
            "site_domain": f.domain,
            "hostnames": [f.domain.clone(), format!("www.{}", f.domain)],
            "date_from": DAY, "date_to": DAY, "limit": 100, "offset": 0, "has_more": false,
        }),
        "{answer}"
    );
}

#[tokio::test]
async fn test_v1_by_campaign_counts_a_visit_for_its_landing_campaign() {
    let f = fixture("v1utm").await;
    let key = key(&f).await;
    let answer = ok(
        &key,
        day(json!({
            "dimensions": ["utm_source"],
            "metrics": ["visitors", "pageviews", "visits"],
            "goals": ["signup"],
        })),
    )
    .await;
    // The signup and the /pricing pageview carry no campaign of their own;
    // they are the linkedin visit's. The dev server's and the fork's linkedin
    // traffic is not the site's. `twitter` is reported as the taxonomy
    // spells it, `x` (FR-102 §3b).
    assert_eq!(
        answer["rows"],
        json!([
            { "dimensions": { "utm_source": "linkedin" }, "metrics": { "visitors": 2, "pageviews": 3, "visits": 2 }, "goals": { "signup": 1 }, "conversion_rates": { "signup": 0.5 } },
            { "dimensions": { "utm_source": "" }, "metrics": { "visitors": 1, "pageviews": 1, "visits": 1 }, "goals": { "signup": 0 }, "conversion_rates": { "signup": 0.0 } },
            { "dimensions": { "utm_source": "x" }, "metrics": { "visitors": 1, "pageviews": 2, "visits": 1 }, "goals": { "signup": 0 }, "conversion_rates": { "signup": 0.0 } },
        ]),
        "{answer}"
    );
}

#[tokio::test]
async fn test_v1_reads_per_blog_path() {
    let f = fixture("v1reads").await;
    let key = key(&f).await;
    let answer = ok(
        &key,
        day(json!({
            "dimensions": ["path"],
            "metrics": ["pageviews", "reads", "median_read_s"],
            "filters": [{ "dimension": "path", "operator": "starts_with", "value": "/blog/" }],
        })),
    )
    .await;
    assert_eq!(
        answer["rows"],
        json!([
            { "dimensions": { "path": "/blog/a" }, "metrics": { "pageviews": 2, "reads": 2, "median_read_s": 40.0 }, "goals": {} },
            { "dimensions": { "path": "/blog/b" }, "metrics": { "pageviews": 1, "reads": 0, "median_read_s": 10.0 }, "goals": {} },
        ]),
        "{answer}"
    );
}

#[tokio::test]
async fn test_v1_visit_metrics_are_null_for_a_page_breakdown() {
    let f = fixture("v1null").await;
    let key = key(&f).await;
    let answer = ok(
        &key,
        day(json!({ "dimensions": ["path"], "metrics": ["visits"] })),
    )
    .await;
    let rows: Vec<(String, Value)> = answer["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["dimensions"]["path"].as_str().unwrap().to_string(),
                r["metrics"]["visits"].clone(),
            )
        })
        .collect();
    assert_eq!(
        rows,
        ["/", "/blog/a", "/blog/b", "/pricing", "/thanks"]
            .into_iter()
            .map(|p| (p.to_string(), Value::Null))
            .collect::<Vec<_>>(),
        "{answer}"
    );

    let by_day = ok(
        &key,
        day(json!({ "dimensions": ["date"], "metrics": ["visits", "visitors"] })),
    )
    .await;
    assert_eq!(
        by_day["rows"],
        json!([{ "dimensions": { "date": DAY }, "metrics": { "visits": 4, "visitors": 4 }, "goals": {} }]),
        "a day is something a visit has: {by_day}"
    );

    let by_entry = ok(
        &key,
        day(json!({ "dimensions": ["entry_page"], "metrics": ["visits", "pageviews"] })),
    )
    .await;
    assert_eq!(
        by_entry["rows"],
        json!([
            { "dimensions": { "entry_page": "/blog/a" }, "metrics": { "visits": 2, "pageviews": 3 }, "goals": {} },
            { "dimensions": { "entry_page": "/" }, "metrics": { "visits": 1, "pageviews": 2 }, "goals": {} },
            { "dimensions": { "entry_page": "/blog/b" }, "metrics": { "visits": 1, "pageviews": 1 }, "goals": {} },
        ]),
        "{by_entry}"
    );
}

#[tokio::test]
async fn test_v1_pages_through_rows() {
    let f = fixture("v1page").await;
    let key = key(&f).await;
    let page = |offset: u64| {
        day(
            json!({ "dimensions": ["path"], "metrics": ["pageviews"], "limit": 2, "offset": offset }),
        )
    };
    let paths = |answer: &Value| -> Vec<String> {
        answer["rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["dimensions"]["path"].as_str().unwrap().to_string())
            .collect()
    };

    let first = ok(&key, page(0)).await;
    assert_eq!(paths(&first), ["/blog/a", "/"], "{first}");
    assert_eq!(first["meta"]["has_more"], true);
    let last = ok(&key, page(4)).await;
    assert_eq!(paths(&last), ["/thanks"], "{last}");
    assert_eq!(last["meta"]["has_more"], false);
    assert_eq!(last["meta"]["offset"], 4);
}

#[tokio::test]
async fn test_v1_refuses_what_the_contract_does_not_allow() {
    let f = fixture("v1bad").await;
    let key = key(&f).await;
    for (body, expect) in [
        (
            day(json!({ "metrics": ["visitors", "revenue"] })),
            "unknown metric",
        ),
        (
            json!({ "date_from": "2025-01-01", "date_to": "2026-02-05", "metrics": ["visitors"] }),
            "366 days",
        ),
        (
            json!({ "date_from": DAY, "date_to": "2026-03-01", "metrics": ["visitors"] }),
            "before",
        ),
        (
            day(json!({ "metrics": ["visitors"], "goals": ["purchase"] })),
            "unknown goal",
        ),
        (
            day(json!({ "metrics": ["visitors"], "dimensions": ["page"] })),
            "unknown dimension",
        ),
        (
            day(json!({ "metrics": ["visitors"], "limit": 1001 })),
            "limit",
        ),
        (
            day(json!({ "metrics": ["visitors"], "site_id": "x" })),
            "unknown field",
        ),
    ] {
        let (status, answer) = v1(Some(&key), &body).await;
        assert_eq!(status, 400, "{body} -> {answer}");
        let error = answer["error"].as_str().unwrap_or_default();
        assert!(error.contains(expect), "{body} -> {answer}");
        assert_eq!(
            answer.as_object().unwrap().len(),
            1,
            "only an error: {answer}"
        );
    }
}
