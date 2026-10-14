//! PS3: engagement per page (reads, read rate, median read time, scroll
//! depth) and the link events of the tracker's opt-in attributes.

use std::time::Duration;

use serde_json::{Value, json};

use crate::helpers::{base_url, user_with_site};
use crate::seed::{self, EventExt, at, ch_id, event, insert};
use crate::trust_test::DAY;

/// An engagement report of a pageview: its id, its time, its properties.
type Report = (
    &'static str,
    &'static str,
    &'static [(&'static str, &'static str)],
);

/// One page, five pageviews, and what each one's reports said.
/// - q1: 10 s at 40 %, then 12 s at 80 %: a read by its depth.
/// - q2: 40 s at 30 %: a read by its time.
/// - q3: 5 s at 50 %: not a read.
/// - q4: 20 s, a page that did not scroll: not a read, and no depth.
/// - q5: 90 % and no read time (scrolled without the focus): a read.
async fn engagement_site(tag: &str) -> (crate::helpers::TestClient, String, String) {
    let (client, org_id, site_id, domain) = user_with_site(tag).await;
    let ch = ch_id(&site_id).await;
    let d = domain.as_str();
    let t = |hms: &str| at(DAY, hms);
    let mut events = Vec::new();
    let reports: [Report; 6] = [
        ("q1", "10:00:10", &[("duration", "10"), ("scroll", "40")]),
        ("q1", "10:00:20", &[("duration", "12"), ("scroll", "80")]),
        ("q2", "11:00:40", &[("duration", "40"), ("scroll", "30")]),
        ("q3", "12:00:05", &[("duration", "5"), ("scroll", "50")]),
        ("q4", "13:00:20", &[("duration", "20")]),
        ("q5", "14:00:30", &[("duration", "0"), ("scroll", "90")]),
    ];
    for (i, pv) in ["q1", "q2", "q3", "q4", "q5"].iter().enumerate() {
        let visitor = format!("w{i}");
        events.push(event(
            ch,
            "pageview",
            d,
            "/post",
            &visitor,
            &format!("s{visitor}"),
            t(&format!("1{i}:00:00")),
        ));
        for (report_pv, hms, props) in &reports {
            if report_pv == pv {
                let mut props = props.to_vec();
                props.push(("pv", *pv));
                events.push(
                    event(
                        ch,
                        "engagement",
                        d,
                        "/post",
                        &visitor,
                        &format!("s{visitor}"),
                        t(hms),
                    )
                    .props(&props),
                );
            }
        }
    }
    // Another page, read for a minute, so the rows sort by their metric.
    events.push(event(
        ch,
        "pageview",
        d,
        "/other",
        "w9",
        "sw9",
        t("15:00:00"),
    ));
    events.push(
        event(ch, "engagement", d, "/other", "w9", "sw9", t("15:01:00"))
            .props(&[("duration", "60"), ("pv", "q9")]),
    );
    insert(&events, &[]).await;
    (client, org_id, site_id)
}

#[tokio::test]
async fn test_a_read_is_thirty_seconds_or_three_quarters_of_the_page() {
    let (client, org_id, site_id) = engagement_site("ps3reads").await;
    let resp = client
        .post(
            &format!("/api/org/{org_id}/site/{site_id}/query"),
            &json!({
                "date_from": DAY, "date_to": DAY,
                "dimensions": ["path"],
                "metrics": ["pageviews", "reads", "read_rate", "median_read_s", "scroll_depth"],
            }),
        )
        .await;
    assert_eq!(resp.status(), 200);
    let answer: Value = resp.json().await.unwrap();
    assert_eq!(
        answer["rows"],
        json!([
            {
                "dimensions": { "path": "/post" },
                // Reads: q1 (80 %), q2 (40 s), q5 (90 %). The median time is
                // of 5, 12, 20 and 40 s; the median depth of 30, 50, 80, 90 %.
                "metrics": { "pageviews": 5, "reads": 3, "read_rate": 0.6, "median_read_s": 16.0, "scroll_depth": 65.0 },
                "goals": {},
            },
            {
                "dimensions": { "path": "/other" },
                "metrics": { "pageviews": 1, "reads": 1, "read_rate": 1.0, "median_read_s": 60.0, "scroll_depth": null },
                "goals": {},
            },
        ]),
        "{answer}"
    );
}

async fn send_event(body: Value) {
    let resp = reqwest::Client::new()
        .post(format!("{}/api/event", base_url()))
        .header(
            "user-agent",
            "Mozilla/5.0 (X11; Linux x86_64; rv:131.0) Gecko/20100101 Firefox/131.0",
        )
        .header("x-forwarded-for", "203.0.113.31")
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), 202, "{body}");
}

/// The `url` property of the site's stored events of a name, once flushed.
async fn stored_urls(ch: u64, name: &str, expect: usize) -> Vec<String> {
    for _ in 0..40 {
        let rows = seed::clickhouse()
            .query(
                "SELECT prop_values[indexOf(prop_keys, 'url')] FROM events \
                 WHERE site_id = ? AND event_name = ? ORDER BY timestamp",
            )
            .bind(ch)
            .bind(name)
            .fetch_all::<String>()
            .await
            .expect("read events");
        if rows.len() >= expect {
            return rows;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    panic!("{name} was never stored");
}

/// A link event stores its address as far as its path, whatever sent it.
#[tokio::test]
async fn test_a_link_events_address_is_stored_without_query_or_credentials() {
    let (_client, _org, site_id, domain) = user_with_site("ps3links").await;
    let ch = ch_id(&site_id).await;
    for (name, url) in [
        (
            "outbound-click",
            "https://user:pw@github.com/gjovanov?token=secret#readme",
        ),
        (
            "file-download",
            "https://cdn.example.net/guide.pdf?signature=abc",
        ),
    ] {
        send_event(json!({
            "domain": domain, "name": name,
            "url": format!("https://{domain}/pricing"), "referrer": "",
            "props": { "url": url },
        }))
        .await;
    }
    assert_eq!(
        stored_urls(ch, "outbound-click", 1).await,
        ["https://github.com/gjovanov"]
    );
    assert_eq!(
        stored_urls(ch, "file-download", 1).await,
        ["https://cdn.example.net/guide.pdf"]
    );
}
