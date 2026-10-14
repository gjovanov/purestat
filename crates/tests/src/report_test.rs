//! PS2: the dashboard's reports (`/query` for a signed-in member, one site or
//! several), the campaign taxonomy of FR-102 §3b on a real ClickHouse, and
//! goal conversions as unique visitors with their rate.

use purestat_db::clickhouse::schemas::{Event, Session};
use purestat_services::analytics::campaign::{
    self, Channel, MEDIUMS, SOURCES, channel_for_referrer_host, medium_group, normalize_medium,
    normalize_source, source_group,
};
use purestat_services::analytics::scope::SiteScope;
use purestat_services::analytics::sql::Sql;
use serde_json::{Value, json};

use crate::helpers::{TestClient, user_with_site};
use crate::seed::{self, EventExt, at, ch_id, event, insert, session};
use crate::trust_test::{DAY, Fixture, fixture};

/// `SELECT v, <exprs of v> FROM (SELECT 7 AS site_id, arrayJoin([values]) AS v)`,
/// every value bound.
async fn evaluate(values: &[String], exprs: &[Sql]) -> Vec<Vec<String>> {
    let mut sql = Sql::new("SELECT [v");
    for expr in exprs {
        sql.push(", ").append(expr);
    }
    sql.push("] FROM (SELECT 7 AS site_id, arrayJoin([");
    for (i, value) in values.iter().enumerate() {
        if i > 0 {
            sql.push(", ");
        }
        sql.bind(value.clone());
    }
    sql.push("]) AS v)");
    sql.query(&seed::clickhouse())
        .fetch_all::<Vec<String>>()
        .await
        .expect("evaluate in ClickHouse")
}

/// The taxonomy is applied in two places, the Rust reference (what roomler-ai
/// runs at its ingress) and the SQL the reports group by. On every value here
/// they must agree: AC5's spellings, whitespace of every kind `str::trim`
/// removes, every canonical value in either case, and unknown values.
#[tokio::test]
async fn test_the_sql_taxonomy_classifies_as_the_rust_one() {
    let mut values: Vec<String> = [
        "YouTub",
        " yt ",
        "YouTube",
        "Twitter",
        "FB",
        "li",
        "IG",
        "Mastodon",
        "check",
        "",
        "   ",
        "\tYT\n",
        "\u{a0}Mastodon\u{2003}",
        "\u{85}li\u{b}",
        "ÄBC",
        "X",
        "LinkedIn",
        " reel ",
        "Video",
        " POST ",
        "youtube.com",
        "news letter",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    for v in SOURCES.iter().chain(MEDIUMS.iter()) {
        values.push(v.to_string());
        values.push(v.to_uppercase());
    }
    let rows = evaluate(
        &values,
        &[
            Sql::new(campaign::normalize_source_sql("v")),
            Sql::new(campaign::source_group_sql("v")),
            Sql::new(campaign::normalize_medium_sql("v")),
            Sql::new(campaign::medium_group_sql("v")),
        ],
    )
    .await;
    assert_eq!(rows.len(), values.len());
    for row in rows {
        let v = &row[0];
        let source = normalize_source(v);
        let medium = normalize_medium(v);
        assert_eq!(
            row[1..],
            [
                source.clone(),
                source_group(&source).to_string(),
                medium.clone(),
                medium_group(&medium).to_string(),
            ],
            "{v:?}"
        );
    }
}

/// The channel of a host, SQL against Rust, with roomler.ai as the site:
/// roomler-ai's pinned cases and the raw forms a host arrives in.
#[tokio::test]
async fn test_the_sql_channel_classifies_as_the_rust_one() {
    let own = vec!["roomler.ai".to_string(), "www.roomler.ai".to_string()];
    let scope = SiteScope {
        ch_id: 7,
        hostnames: own.clone(),
    };
    let hosts: Vec<String> = [
        "google.com",
        "www.google.com",
        "www.google.co.uk",
        "google.de",
        "bing.com",
        "www.bing.com",
        "duckduckgo.com",
        "ecosia.org",
        "yandex.ru",
        "www.yandex.com",
        "chatgpt.com",
        "chat.openai.com",
        "perplexity.ai",
        "www.perplexity.ai",
        "gemini.google.com",
        "copilot.microsoft.com",
        "claude.ai",
        "t.co",
        "x.com",
        "twitter.com",
        "linkedin.com",
        "www.linkedin.com",
        "lnkd.in",
        "facebook.com",
        "l.facebook.com",
        "m.facebook.com",
        "youtube.com",
        "www.youtube.com",
        "m.youtube.com",
        "youtu.be",
        "tiktok.com",
        "www.tiktok.com",
        "reddit.com",
        "old.reddit.com",
        "out.reddit.com",
        "instagram.com",
        "news.ycombinator.com",
        "lobste.rs",
        "dev.to",
        "hashnode.com",
        "medium.com",
        "github.com",
        "roomler.ai",
        "www.roomler.ai",
        "docs.roomler.ai",
        "example.com",
        "localhost",
        "localhost:5000",
        "googleusercontent.com",
        "notgoogle.com",
        "google",
        "roomler.ai.evil.example",
        "WWW.YouTube.com:443",
        "github.com.",
        "",
        "   ",
        ":8080",
        "...",
        "a..google.com",
        "com.google.android.gm",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let rows = evaluate(&hosts, &[campaign::channel_sql("v", &[scope])]).await;
    assert_eq!(rows.len(), hosts.len());
    for row in rows {
        assert_eq!(
            row[1],
            channel_for_referrer_host(Some(&row[0]), &own).as_str(),
            "{:?}",
            row[0]
        );
    }
}

/// From a stored referrer to its channel: the host is read from the
/// address, and a referrer the table cannot read is `other`, never `direct`.
#[tokio::test]
async fn test_a_referrer_address_reads_as_its_hosts_channel() {
    let scope = SiteScope {
        ch_id: 7,
        hostnames: vec!["roomler.ai".into(), "www.roomler.ai".into()],
    };
    let cases = [
        ("https://www.google.co.uk/", Channel::Search),
        ("https://l.facebook.com/l.php", Channel::Social),
        ("https://user@claude.ai:443/chat", Channel::AiAssistant),
        ("https://github.com./gjovanov", Channel::Code),
        ("https://roomler.ai/blog/a", Channel::SelfSite),
        ("https://www.roomler.ai/", Channel::SelfSite),
        ("http://localhost:5000/", Channel::Other),
        ("https://[2001:db8::1]:8080/", Channel::Other),
        ("garbage", Channel::Other),
        ("https:///path", Channel::Other),
        ("roomler.ai", Channel::SelfSite),
        (
            "https://www.google.com/search?q=private#frag",
            Channel::Search,
        ),
        ("android-app://com.google.android.gm/", Channel::Search),
        ("", Channel::Direct),
    ];
    let referrers: Vec<String> = cases.iter().map(|(r, _)| r.to_string()).collect();
    let host = campaign::referrer_host_sql("v");
    let rows = evaluate(&referrers, &[campaign::channel_sql(&host, &[scope])]).await;
    for (row, (referrer, expected)) in rows.iter().zip(cases) {
        assert_eq!(row[1], expected.as_str(), "{referrer:?}");
    }
}

async fn query(f: &Fixture, body: Value) -> Value {
    let resp = f
        .client
        .post(
            &format!("/api/org/{}/site/{}/query", f.org_id, f.site_id),
            &body,
        )
        .await;
    let status = resp.status();
    let answer: Value = resp.json().await.unwrap_or_default();
    assert_eq!(status, 200, "{body} -> {answer}");
    answer
}

fn day(extra: Value) -> Value {
    let mut body = json!({ "date_from": DAY, "date_to": DAY });
    for (k, v) in extra.as_object().unwrap() {
        body[k] = v.clone();
    }
    body
}

/// `(value, visitors)` of each row of a one-dimension answer.
fn rows_of(answer: &Value, dimension: &str) -> Vec<(String, u64)> {
    answer["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .map(|r| {
            (
                r["dimensions"][dimension].as_str().unwrap().to_string(),
                r["metrics"]["visitors"].as_u64().unwrap(),
            )
        })
        .collect()
}

fn with_referrer(mut e: Event, referrer: &str) -> Event {
    e.referrer = referrer.to_string();
    e
}

fn visit(s: Session, referrer: &str, utm_source: &str, utm_medium: &str) -> Session {
    Session {
        referrer: referrer.to_string(),
        utm_source: utm_source.to_string(),
        utm_medium: utm_medium.to_string(),
        ..s
    }
}

/// The dashboard reads the same rows an API key does: goals as unique
/// visitors who completed them, and the rate over the row's visitors.
#[tokio::test]
async fn test_goals_report_converting_visitors_and_their_rate() {
    let f = fixture("rpgoals").await;
    let answer = query(
        &f,
        day(json!({ "metrics": ["visitors"], "goals": ["signup", "thanks"] })),
    )
    .await;
    assert_eq!(
        answer["rows"],
        json!([{
            "dimensions": {},
            "metrics": { "visitors": 4 },
            "goals": { "signup": 1, "thanks": 1 },
            "conversion_rates": { "signup": 0.25, "thanks": 0.25 },
        }]),
        "{answer}"
    );
    // Per campaign: the signup is the linkedin visit's.
    let by_source = query(
        &f,
        day(json!({ "dimensions": ["utm_source"], "metrics": ["visitors"], "goals": ["signup"] })),
    )
    .await;
    let linkedin = &by_source["rows"][0];
    assert_eq!(
        linkedin["dimensions"]["utm_source"], "linkedin",
        "{by_source}"
    );
    assert_eq!(linkedin["goals"]["signup"], 1);
    assert_eq!(linkedin["conversion_rates"]["signup"], 0.5);
}

/// A site with a day of visits from every kind of place, and one visit still
/// in progress (its events, no `sessions` row yet).
async fn channel_fixture(tag: &str) -> Fixture {
    let (client, org_id, site_id, domain) = user_with_site(tag).await;
    let ch = ch_id(&site_id).await;
    let d = domain.as_str();
    let t = |hms: &str| at(DAY, hms);
    let visits = [
        ("v1", "https://www.google.de/", "", ""),
        ("v2", "https://l.facebook.com/l.php", "YouTub", "Video"),
        ("v3", "https://chatgpt.com/", " yt ", "video"),
        ("v4", "", "YouTube", "reel"),
        ("v5", &format!("https://www.{d}/pricing"), "Mastodon", ""),
        ("v6", "https://googleusercontent.com/x", "Twitter", "post"),
        ("v7", "https://news.ycombinator.com/item", "", ""),
    ];
    let mut events = Vec::new();
    let mut sessions = Vec::new();
    for (i, (v, referrer, source, medium)) in visits.iter().enumerate() {
        let s = format!("s{v}");
        let hms = format!("1{i}:00:00");
        events.push(with_referrer(
            event(ch, "pageview", d, "/", v, &s, t(&hms)).utm_source(source),
            referrer,
        ));
        sessions.push(visit(
            session(ch, v, &s, "/", "/", 1, 0, source, t(&hms)),
            referrer,
            source,
            medium,
        ));
    }
    // In progress: no visit row, so the event's own referrer counts.
    events.push(with_referrer(
        event(ch, "pageview", d, "/live", "v8", "sv8", t("20:00:00")),
        "https://github.com/gjovanov",
    ));
    insert(&events, &sessions).await;
    Fixture {
        client,
        org_id,
        site_id,
        domain,
        ch_id: ch,
    }
}

#[tokio::test]
async fn test_channels_group_visits_by_their_referrer() {
    let f = channel_fixture("rpchan").await;
    let answer = query(
        &f,
        day(json!({ "dimensions": ["channel"], "metrics": ["visitors", "visits"] })),
    )
    .await;
    let mut rows = rows_of(&answer, "channel");
    rows.sort();
    assert_eq!(
        rows,
        [
            ("ai_assistant".to_string(), 1),
            ("code".to_string(), 1),
            ("community".to_string(), 1),
            ("direct".to_string(), 1),
            ("other".to_string(), 1),
            ("search".to_string(), 1),
            ("self".to_string(), 1),
            ("social".to_string(), 1),
        ],
        "{answer}"
    );
    // A filter on the channel narrows the whole query.
    let search = query(
        &f,
        day(json!({
            "metrics": ["visitors", "visits"],
            "filters": [{ "dimension": "channel", "operator": "is", "value": "search" }],
        })),
    )
    .await;
    assert_eq!(
        search["rows"][0]["metrics"],
        json!({ "visitors": 1, "visits": 1 }),
        "{search}"
    );
}

#[tokio::test]
async fn test_campaign_sources_are_normalised_and_unknown_ones_kept() {
    let f = channel_fixture("rputm").await;
    let sources = query(
        &f,
        day(json!({
            "dimensions": ["utm_source"], "metrics": ["visitors", "visits"],
            "filters": [{ "dimension": "utm_source", "operator": "is_not", "value": "" }],
        })),
    )
    .await;
    assert_eq!(
        rows_of(&sources, "utm_source"),
        [
            ("youtube".to_string(), 3),
            ("mastodon".to_string(), 1),
            ("x".to_string(), 1),
        ],
        "YouTub, ' yt ' and YouTube are one source; Mastodon is kept as itself: {sources}"
    );
    let groups = query(
        &f,
        day(json!({
            "dimensions": ["utm_source_group"], "metrics": ["visitors"],
            "filters": [{ "dimension": "utm_source", "operator": "is_not", "value": "" }],
        })),
    )
    .await;
    assert_eq!(
        rows_of(&groups, "utm_source_group"),
        [
            ("youtube".to_string(), 3),
            ("other".to_string(), 1),
            ("x".to_string(), 1),
        ],
        "an unknown source is reported under other, not dropped: {groups}"
    );
    let mediums = query(
        &f,
        day(json!({
            "dimensions": ["utm_medium", "utm_medium_group"], "metrics": ["visits"],
            "filters": [{ "dimension": "utm_medium", "operator": "is_not", "value": "" }],
        })),
    )
    .await;
    let mediums: Vec<(String, String, u64)> = mediums["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["dimensions"]["utm_medium"].as_str().unwrap().to_string(),
                r["dimensions"]["utm_medium_group"]
                    .as_str()
                    .unwrap()
                    .to_string(),
                r["metrics"]["visits"].as_u64().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        mediums,
        [
            ("video".to_string(), "video".to_string(), 2),
            ("post".to_string(), "post".to_string(), 1),
            ("reel".to_string(), "other".to_string(), 1),
        ]
    );
}

#[tokio::test]
async fn test_the_report_is_for_members_of_the_sites_org() {
    let f = fixture("rpauth").await;
    let (outsider, outsider_org, _site, _domain) = user_with_site("rpout").await;
    let body = day(json!({ "metrics": ["visitors"] }));

    let resp = outsider
        .post(
            &format!("/api/org/{}/site/{}/query", f.org_id, f.site_id),
            &body,
        )
        .await;
    assert_eq!(resp.status(), 403, "not a member of the site's org");

    let resp = outsider
        .post(
            &format!("/api/org/{outsider_org}/site/{}/query", f.site_id),
            &body,
        )
        .await;
    assert_eq!(resp.status(), 404, "another org's site in one's own org");

    let resp = outsider
        .post(
            &format!(
                "/api/org/{outsider_org}/analytics/query?site_ids={}",
                f.site_id
            ),
            &body,
        )
        .await;
    assert_eq!(resp.status(), 400, "another org's site among the sites");

    let resp = TestClient::new()
        .post(
            &format!("/api/org/{}/site/{}/query", f.org_id, f.site_id),
            &body,
        )
        .await;
    assert_eq!(resp.status(), 401, "signed out");

    for (bad, expect) in [
        (json!({ "metrics": ["visitors"] }), "date_from"),
        (
            day(json!({ "metrics": ["visitors"], "site_id": "x" })),
            "unknown field",
        ),
        (
            day(json!({ "metrics": ["visitors"], "goals": ["purchase"] })),
            "unknown goal",
        ),
    ] {
        let resp = f
            .client
            .post(
                &format!("/api/org/{}/site/{}/query", f.org_id, f.site_id),
                &bad,
            )
            .await;
        assert_eq!(resp.status(), 400, "{bad}");
        let answer: Value = resp.json().await.unwrap();
        assert!(
            answer["message"]
                .as_str()
                .unwrap_or_default()
                .contains(expect),
            "{bad} -> {answer}"
        );
    }
}

/// Several sites of one org read as one, each through its own hostnames;
/// goals belong to a site, so the org's query names none.
#[tokio::test]
async fn test_several_sites_of_an_org_are_one_report() {
    let f = fixture("rpmulti").await;
    let db = purestat_db::connect(&seed::settings()).await.unwrap();
    db.collection::<bson::Document>("orgs")
        .update_one(
            bson::doc! { "_id": bson::oid::ObjectId::parse_str(&f.org_id).unwrap() },
            bson::doc! { "$set": { "limits.max_sites": 5 } },
        )
        .await
        .unwrap();
    let second_domain = format!("second-{}", f.domain);
    let second: Value = f
        .client
        .post(
            &format!("/api/org/{}/site", f.org_id),
            &json!({ "domain": second_domain, "name": "second" }),
        )
        .await
        .json()
        .await
        .unwrap();
    let second_id = second["id"].as_str().expect("a second site").to_string();
    let ch2 = ch_id(&second_id).await;
    let t = |hms: &str| at(DAY, hms);
    insert(
        &[
            event(
                ch2,
                "pageview",
                &second_domain,
                "/",
                "w1",
                "sw1",
                t("09:00:00"),
            ),
            event(
                ch2,
                "pageview",
                &second_domain,
                "/",
                "w2",
                "sw2",
                t("09:30:00"),
            ),
            // The first site's hostname under the second site's id is not the second site's.
            event(ch2, "pageview", &f.domain, "/", "w3", "sw3", t("09:45:00")),
        ],
        &[
            session(ch2, "w1", "sw1", "/", "/", 1, 0, "", t("09:00:00")),
            session(ch2, "w2", "sw2", "/", "/", 1, 0, "", t("09:30:00")),
        ],
    )
    .await;

    let both = format!("{},{}", f.site_id, second_id);
    let resp = f
        .client
        .post(
            &format!("/api/org/{}/analytics/query?site_ids={both}", f.org_id),
            &day(json!({ "metrics": ["visitors", "visits", "pageviews"] })),
        )
        .await;
    assert_eq!(resp.status(), 200);
    let answer: Value = resp.json().await.unwrap();
    assert_eq!(
        answer["rows"][0]["metrics"],
        json!({ "visitors": 6, "visits": 6, "pageviews": 8 }),
        "the fixture's 4 visitors and the second site's 2: {answer}"
    );
    assert_eq!(answer["meta"]["sites"], 2);

    let resp = f
        .client
        .post(
            &format!("/api/org/{}/analytics/query?site_ids={both}", f.org_id),
            &day(json!({ "metrics": ["visitors"], "goals": ["signup"] })),
        )
        .await;
    assert_eq!(resp.status(), 400, "goals belong to a site");
}
