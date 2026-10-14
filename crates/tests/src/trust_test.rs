//! PS0a: what reaches the numbers. Ingest keeps only a visitor's events on the
//! site's own hostnames; every read counts only those hostnames and only
//! pageview events as pageviews; the entry page reads visits; a new site gets
//! a ClickHouse id of its own.

use std::time::Duration;

use crate::helpers::{TestClient, base_url, user_with_site};
use crate::seed::{self, EventExt, at, ch_id, event, insert, session};

/// The day the fixed fixture is on.
pub const DAY: &str = "2026-03-10";

/// A site with a fixed day of traffic, and traffic that is not the site's.
///
/// On the site's hostnames, on DAY: four visits by four visitors.
/// - s1 (linkedin): /blog/a, read 45 s over two reports; /pricing; signup.
/// - s2 (linkedin, on www.): /blog/b, read 10 s.
/// - s3 (twitter): /, a 60 s report from before pageview ids; /thanks.
/// - s4 (no campaign): /blog/a, read 35 s.
///
/// So: 4 visitors, 6 pageviews, 4 visits (2 bounces, 420 s in total),
/// reads of 10, 35, 45 and 60 s, one signup and one /thanks.
///
/// Not the site's: a dev server and a fork with linkedin traffic and a
/// signup, and a visit on the next day.
pub struct Fixture {
    pub client: TestClient,
    pub org_id: String,
    pub site_id: String,
    pub domain: String,
    pub ch_id: u64,
}

pub async fn fixture(tag: &str) -> Fixture {
    let (client, org_id, site_id, domain) = user_with_site(tag).await;
    let ch = ch_id(&site_id).await;
    seed::goal(
        &client,
        &org_id,
        &site_id,
        serde_json::json!({ "goal_type": "custom_event", "name": "signup", "event_name": "signup" }),
    )
    .await;
    seed::goal(
        &client,
        &org_id,
        &site_id,
        serde_json::json!({ "goal_type": "pageview", "name": "thanks", "page_path": "/thanks" }),
    )
    .await;

    let d = domain.as_str();
    let www = format!("www.{domain}");
    let t = |hms: &str| at(DAY, hms);
    let events = vec![
        event(ch, "pageview", d, "/blog/a", "v1", "s1", t("10:00:00")).utm_source("linkedin"),
        event(ch, "engagement", d, "/blog/a", "v1", "s1", t("10:00:20"))
            .utm_source("linkedin")
            .props(&[("duration", "20"), ("pv", "p1")]),
        event(ch, "engagement", d, "/blog/a", "v1", "s1", t("10:00:50"))
            .utm_source("linkedin")
            .props(&[("duration", "45"), ("pv", "p1")]),
        event(ch, "pageview", d, "/pricing", "v1", "s1", t("10:01:00")),
        event(ch, "signup", d, "/pricing", "v1", "s1", t("10:05:00")),
        event(ch, "pageview", &www, "/blog/b", "v2", "s2", t("11:00:00")).utm_source("linkedin"),
        event(ch, "engagement", &www, "/blog/b", "v2", "s2", t("11:00:10"))
            .utm_source("linkedin")
            .props(&[("duration", "10"), ("pv", "p2")]),
        event(ch, "pageview", d, "/", "v3", "s3", t("12:00:00")).utm_source("twitter"),
        event(ch, "engagement", d, "/", "v3", "s3", t("12:01:00"))
            .utm_source("twitter")
            .props(&[("duration", "60")]),
        event(ch, "pageview", d, "/thanks", "v3", "s3", t("12:02:00")),
        event(ch, "pageview", d, "/blog/a", "v4", "s4", t("13:00:00")),
        event(ch, "engagement", d, "/blog/a", "v4", "s4", t("13:00:40"))
            .props(&[("duration", "35"), ("pv", "p4")]),
        // Not the site's.
        event(
            ch,
            "pageview",
            "localhost",
            "/blog/a",
            "v9",
            "s9",
            t("14:00:00"),
        )
        .utm_source("linkedin"),
        event(
            ch,
            "engagement",
            "localhost",
            "/blog/a",
            "v9",
            "s9",
            t("14:01:00"),
        )
        .props(&[("duration", "500"), ("pv", "p9")]),
        event(
            ch,
            "signup",
            "localhost",
            "/register",
            "v9",
            "s9",
            t("14:02:00"),
        ),
        event(
            ch,
            "pageview",
            "evil.example",
            "/thanks",
            "v8",
            "s8",
            t("15:00:00"),
        )
        .utm_source("linkedin"),
        event(
            ch,
            "pageview",
            d,
            "/blog/a",
            "v5",
            "s5",
            at("2026-03-11", "09:00:00"),
        ),
    ];
    let sessions = vec![
        session(
            ch,
            "v1",
            "s1",
            "/blog/a",
            "/pricing",
            2,
            300,
            "linkedin",
            t("10:00:00"),
        ),
        session(
            ch,
            "v2",
            "s2",
            "/blog/b",
            "/blog/b",
            1,
            0,
            "linkedin",
            t("11:00:00"),
        ),
        session(
            ch,
            "v3",
            "s3",
            "/",
            "/thanks",
            2,
            120,
            "twitter",
            t("12:00:00"),
        ),
        session(
            ch,
            "v4",
            "s4",
            "/blog/a",
            "/blog/a",
            1,
            0,
            "",
            t("13:00:00"),
        ),
        session(
            ch,
            "v9",
            "s9",
            "/blog/a",
            "/register",
            1,
            120,
            "linkedin",
            t("14:00:00"),
        ),
        session(
            ch,
            "v8",
            "s8",
            "/thanks",
            "/thanks",
            1,
            0,
            "linkedin",
            t("15:00:00"),
        ),
        session(
            ch,
            "v5",
            "s5",
            "/blog/a",
            "/blog/a",
            1,
            0,
            "",
            at("2026-03-11", "09:00:00"),
        ),
    ];
    insert(&events, &sessions).await;

    Fixture {
        client,
        org_id,
        site_id,
        domain,
        ch_id: ch,
    }
}

async fn stats(f: &Fixture, body: serde_json::Value) -> serde_json::Value {
    let resp = f
        .client
        .post(
            &format!("/api/org/{}/site/{}/stats", f.org_id, f.site_id),
            &body,
        )
        .await;
    assert_eq!(resp.status(), 200, "stats {body}");
    resp.json().await.unwrap()
}

/// The (value, visitors, pageviews) rows of a dashboard breakdown.
fn breakdown(body: &serde_json::Value) -> Vec<(String, u64, u64)> {
    body["dimensions"]
        .as_array()
        .expect("a breakdown")
        .iter()
        .map(|r| {
            (
                r["value"].as_str().unwrap().to_string(),
                r["metrics"]["visitors"].as_u64().unwrap(),
                r["metrics"]["pageviews"].as_u64().unwrap(),
            )
        })
        .collect()
}

#[tokio::test]
async fn test_pageviews_count_pageview_events_only() {
    let f = fixture("pageviews").await;
    let totals = stats(
        &f,
        serde_json::json!({ "date_from": DAY, "date_to": DAY, "metrics": ["visitors", "pageviews", "bounce_rate", "visit_duration"] }),
    )
    .await;
    assert_eq!(
        totals["metrics"]["pageviews"], 6,
        "engagement and goal rows are not pageviews: {totals}"
    );
    assert_eq!(totals["metrics"]["visitors"], 4, "{totals}");
    assert_eq!(totals["metrics"]["bounce_rate"], 50.0, "{totals}");
    assert_eq!(totals["metrics"]["visit_duration"], 105.0, "{totals}");

    let series = stats(
        &f,
        serde_json::json!({ "date_from": DAY, "date_to": DAY, "metrics": ["visitors", "pageviews"], "interval": "day" }),
    )
    .await;
    assert_eq!(
        series["timeseries"][0]["metrics"]["pageviews"], 6,
        "{series}"
    );
}

#[tokio::test]
async fn test_reads_count_only_the_sites_hostnames() {
    let f = fixture("scoped").await;
    let pages = stats(
        &f,
        serde_json::json!({ "date_from": DAY, "date_to": DAY, "metrics": ["visitors"], "dimensions": ["page"] }),
    )
    .await;
    assert_eq!(
        breakdown(&pages),
        [
            ("/blog/a".to_string(), 2, 2),
            ("/".to_string(), 1, 1),
            ("/blog/b".to_string(), 1, 1),
            ("/pricing".to_string(), 1, 1),
            ("/thanks".to_string(), 1, 1),
        ],
        "localhost's /blog/a and the fork's /thanks are not the site's: {pages}"
    );

    let hosts = stats(
        &f,
        serde_json::json!({ "date_from": DAY, "date_to": DAY, "metrics": ["visitors"], "dimensions": ["hostname"] }),
    )
    .await;
    let www = format!("www.{}", f.domain);
    assert_eq!(
        breakdown(&hosts),
        [(f.domain.clone(), 3, 5), (www, 1, 1)],
        "{hosts}"
    );

    let export = f
        .client
        .get(&format!(
            "/api/org/{}/site/{}/export?date_from={DAY}&date_to={DAY}",
            f.org_id, f.site_id
        ))
        .await;
    assert_eq!(export.status(), 200, "export");
    let csv = export.text().await.unwrap();
    let lines: Vec<&str> = csv.lines().collect();
    assert_eq!(
        lines.len(),
        1 + 12,
        "a header and the site's 12 events:\n{csv}"
    );
    assert!(
        !csv.contains("/register"),
        "the dev server's signup:\n{csv}"
    );

    // Realtime: the last five minutes, pageviews on the site's hostnames.
    let now = time::OffsetDateTime::now_utc();
    insert(
        &[
            event(f.ch_id, "pageview", &f.domain, "/live", "rt1", "rts1", now),
            event(
                f.ch_id,
                "pageview",
                "localhost",
                "/live",
                "rt2",
                "rts2",
                now,
            ),
            event(
                f.ch_id,
                "engagement",
                &f.domain,
                "/live",
                "rt3",
                "rts3",
                now,
            )
            .props(&[("duration", "5"), ("pv", "rtp")]),
        ],
        &[],
    )
    .await;
    let realtime: serde_json::Value = f
        .client
        .get(&format!(
            "/api/org/{}/site/{}/realtime",
            f.org_id, f.site_id
        ))
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(realtime["current_visitors"], 1, "{realtime}");
    assert_eq!(
        realtime["top_pages"],
        serde_json::json!([{ "path": "/live", "visitors": 1 }]),
        "{realtime}"
    );
}

#[tokio::test]
async fn test_the_entry_page_breakdown_reads_visits() {
    let f = fixture("entry").await;
    let entry = stats(
        &f,
        serde_json::json!({ "date_from": DAY, "date_to": DAY, "metrics": ["visitors"], "dimensions": ["entry_page"] }),
    )
    .await;
    assert_eq!(
        breakdown(&entry),
        [
            ("/blog/a".to_string(), 2, 3),
            ("/".to_string(), 1, 2),
            ("/blog/b".to_string(), 1, 1),
        ],
        "{entry}"
    );
    let exit = stats(
        &f,
        serde_json::json!({ "date_from": DAY, "date_to": DAY, "metrics": ["visitors"], "dimensions": ["exit_page"] }),
    )
    .await;
    assert_eq!(
        breakdown(&exit),
        [
            ("/blog/a".to_string(), 1, 1),
            ("/blog/b".to_string(), 1, 1),
            ("/pricing".to_string(), 1, 2),
            ("/thanks".to_string(), 1, 2),
        ],
        "{exit}"
    );
}

/// The paths stored under a site, oldest first.
async fn stored_paths(ch_id: u64) -> Vec<String> {
    seed::clickhouse()
        .query("SELECT path FROM events WHERE site_id = ? ORDER BY timestamp, path")
        .bind(ch_id)
        .fetch_all::<String>()
        .await
        .expect("read events")
}

async fn send_event(user_agent: &str, forwarded_for: &str, body: serde_json::Value) {
    let resp = reqwest::Client::new()
        .post(format!("{}/api/event", base_url()))
        .header("user-agent", user_agent)
        .header("x-forwarded-for", forwarded_for)
        .json(&body)
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), 202, "every event is answered 202: {body}");
}

/// Waits for ingest's flush until `path` is stored, and returns what is.
async fn wait_for(ch_id: u64, path: &str) -> Vec<String> {
    for _ in 0..40 {
        let paths = stored_paths(ch_id).await;
        if paths.iter().any(|p| p == path) {
            return paths;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    panic!("{path} was never stored");
}

const BROWSER: &str = "Mozilla/5.0 (X11; Linux x86_64; rv:131.0) Gecko/20100101 Firefox/131.0";

#[tokio::test]
async fn test_ingest_keeps_only_visitors_on_the_sites_hostnames() {
    let (_client, _org, site_id, domain) = user_with_site("ingest").await;
    let ch = ch_id(&site_id).await;
    let page = |url: String| serde_json::json!({ "domain": domain, "name": "pageview", "url": url, "referrer": "" });

    send_event(
        "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)",
        "203.0.113.10",
        page(format!("https://{domain}/bot")),
    )
    .await;
    send_event(
        "curl/8.5.0",
        "203.0.113.11",
        page(format!("https://{domain}/curl")),
    )
    .await;
    send_event(
        BROWSER,
        "203.0.113.12",
        page("http://localhost:5173/dev".to_string()),
    )
    .await;
    send_event(
        BROWSER,
        "203.0.113.13",
        page("http://192.168.1.20/lan".to_string()),
    )
    .await;
    send_event(
        BROWSER,
        "203.0.113.14",
        page("https://evil.example/fork".to_string()),
    )
    .await;
    send_event(
        BROWSER,
        "203.0.113.15",
        page(format!("https://WWW.{domain}/real")),
    )
    .await;

    let stored = wait_for(ch, "/real").await;
    assert_eq!(
        stored,
        ["/real"],
        "only the visitor on the site's own hostname is stored"
    );
    let hosts = seed::clickhouse()
        .query("SELECT hostname FROM events WHERE site_id = ?")
        .bind(ch)
        .fetch_all::<String>()
        .await
        .unwrap();
    assert_eq!(
        hosts,
        [format!("www.{domain}")],
        "stored as the reads compare it"
    );
}

#[tokio::test]
async fn test_ingest_reads_the_client_address_as_the_limiter_does() {
    let (_client, _org, site_id, domain) = user_with_site("clientip").await;
    let ch = ch_id(&site_id).await;
    let page = |path: &str| serde_json::json!({ "domain": domain, "name": "pageview", "url": format!("https://{domain}{path}"), "referrer": "" });

    // The proxy appended 198.51.100.7; the entries on its left are the
    // client's own, and must not change who the visitor is.
    send_event(BROWSER, "1.1.1.1, 198.51.100.7", page("/one")).await;
    send_event(BROWSER, "9.9.9.9, 198.51.100.7, 10.0.0.2", page("/two")).await;
    wait_for(ch, "/two").await;
    let visitors = seed::clickhouse()
        .query("SELECT uniqExact(visitor_hash) FROM events WHERE site_id = ?")
        .bind(ch)
        .fetch_one::<u64>()
        .await
        .unwrap();
    assert_eq!(
        visitors, 1,
        "one client, whatever it wrote into X-Forwarded-For"
    );
}

#[tokio::test]
async fn test_two_sites_created_in_the_same_second_get_different_ids() {
    for _ in 0..5 {
        let (a, b) = tokio::join!(user_with_site("sameid-a"), user_with_site("sameid-b"));
        let (site_a, site_b) = tokio::join!(seed::site(&a.2), seed::site(&b.2));
        let (id_a, id_b) = (site_a.id.unwrap(), site_b.id.unwrap());
        if id_a.timestamp() != id_b.timestamp() {
            continue; // the two creations straddled a second; try again
        }
        assert_ne!(
            site_a.clickhouse_id(),
            site_b.clickhouse_id(),
            "each site has a ClickHouse id of its own"
        );
        assert!(
            site_a.ch_id.is_some() && site_b.ch_id.is_some(),
            "a new site is assigned an id"
        );
        return;
    }
    panic!("could not create two sites within one second");
}

#[tokio::test]
async fn test_oauth_routes_are_not_mounted_by_default() {
    let resp = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap()
        .get(format!("{}/api/oauth/google", base_url()))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(
        resp.status(),
        404,
        "OAuth sign-in is off unless the deployment sets PURESTAT__OAUTH__ENABLED=true"
    );
}
