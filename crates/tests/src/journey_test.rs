//! PS5: journeys. The paths from an entry page and into a goal, against a
//! fixed fixture in ClickHouse, and the k-anonymity rule: a path followed by
//! fewer than 5 distinct visitors is never returned by the query, and its
//! visits fold into "other".

use serde_json::{Value, json};

use crate::helpers::{TestClient, user_with_site};
use crate::seed::{self, EventExt, at, ch_id, event, insert};
use purestat_services::analytics::journey::{JourneyQuery, JourneyRequest, MIN_VISITORS};
use purestat_services::analytics::query::QueryService;
use purestat_services::analytics::scope::SiteScope;

const DAY: &str = "2026-05-12";
const DAY_BEFORE: &str = "2026-05-11";

/// A site with two goals (`signup`, a custom event; `thanks`, a pageview of
/// /thanks) and a day of visits.
///
/// Entering on /start (25 visits by 22 visitors):
/// - A, 6 visitors: /start, /a, /b. One reloads /a and sends engagement
///   reports, which change nothing.
/// - B, 4 visitors: /start, /c. Too few: folds into "other".
/// - C, 5 visits by 2 visitors: /start, /d. Five visits are not five
///   visitors: folds into "other".
/// - D, 5 visitors: /start and nothing else.
/// - E, 5 visitors: /start, /a, /e, /f, /g. Exactly five: shown.
///
/// Not entering on /start, or not the site's: 5 visitors who entered on
/// /other and then saw /start; 5 on localhost (/start, /zzz); 5 whose visit
/// started at 23:59 the day before (/start) and went on to /late after
/// midnight.
///
/// Into `signup` (15 visits):
/// - G1, 6 visitors: /pricing, /register, signup, then /dashboard.
/// - G2, 5 visitors: /blog, signup.
/// - G3, 3 visitors: /x, /y, signup. Too few.
/// - G4, 1 visitor: signup before any pageview, then /after.
/// - 5 visitors saw /pricing and /register and did not sign up.
///
/// Into `thanks`, 5 visitors: /a, /thanks, /thanks.
struct Fixture {
    client: TestClient,
    org_id: String,
    site_id: String,
    ch_id: u64,
    domain: String,
    signup: String,
    thanks: String,
}

async fn goal(client: &TestClient, org_id: &str, site_id: &str, body: Value) -> String {
    let resp = client
        .post(&format!("/api/org/{org_id}/site/{site_id}/goal"), &body)
        .await;
    assert_eq!(resp.status(), 201, "creating a goal");
    resp.json::<Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string()
}

/// One visit: its pages (or `!name` for a custom event) a minute apart.
fn visit(
    ch: u64,
    host: &str,
    visitor: &str,
    session: &str,
    day: &str,
    start: u32,
    steps: &[&str],
) -> Vec<purestat_db::clickhouse::schemas::Event> {
    steps
        .iter()
        .enumerate()
        .map(|(i, step)| {
            let minute = start + i as u32;
            let ts = at(day, &format!("{:02}:{:02}:00", minute / 60, minute % 60));
            match step.strip_prefix('!') {
                Some(name) => event(ch, name, host, "/", visitor, session, ts),
                None => event(ch, "pageview", host, step, visitor, session, ts),
            }
        })
        .collect()
}

async fn fixture(tag: &str) -> Fixture {
    let (client, org_id, site_id, domain) = user_with_site(tag).await;
    let ch = ch_id(&site_id).await;
    let signup = goal(
        &client,
        &org_id,
        &site_id,
        json!({ "goal_type": "custom_event", "name": "Signup", "event_name": "signup" }),
    )
    .await;
    let thanks = goal(
        &client,
        &org_id,
        &site_id,
        json!({ "goal_type": "pageview", "name": "Thanks", "page_path": "/thanks" }),
    )
    .await;

    let d = domain.as_str();
    let mut events = Vec::new();
    let mut add = |prefix: &str, n: usize, start: u32, steps: &[&str]| {
        for i in 0..n {
            let id = format!("{prefix}{i}");
            events.extend(visit(ch, d, &id, &id, DAY, start + 10 * i as u32, steps));
        }
    };
    add("a", 6, 60, &["/start", "/a", "/b"]);
    add("b", 4, 180, &["/start", "/c"]);
    add("d", 5, 300, &["/start"]);
    add("e", 5, 360, &["/start", "/a", "/e", "/f", "/g"]);
    add("o", 5, 420, &["/other", "/start", "/a"]);
    add(
        "g1-",
        6,
        480,
        &["/pricing", "/register", "!signup", "/dashboard"],
    );
    add("g2-", 5, 560, &["/blog", "!signup"]);
    add("g3-", 3, 620, &["/x", "/y", "!signup"]);
    add("g4-", 1, 660, &["!signup", "/after"]);
    add("n", 5, 680, &["/pricing", "/register"]);
    add("t", 5, 740, &["/a", "/thanks", "/thanks"]);
    // C: five visits by two visitors.
    for (visitor, session) in [
        ("w1", "c1"),
        ("w1", "c2"),
        ("w1", "c3"),
        ("w2", "c4"),
        ("w2", "c5"),
    ] {
        events.extend(visit(ch, d, visitor, session, DAY, 240, &["/start", "/d"]));
    }
    // A reload of /a and engagement reports in one of the A visits.
    events.push(event(
        ch,
        "pageview",
        d,
        "/a",
        "a0",
        "a0",
        at(DAY, "01:01:30"),
    ));
    events.push(
        event(ch, "engagement", d, "/a", "a0", "a0", at(DAY, "01:01:40"))
            .props(&[("duration", "40"), ("pv", "p")]),
    );
    // Not the site's, and visits of the day before.
    for i in 0..5 {
        let id = format!("l{i}");
        events.extend(visit(
            ch,
            "localhost",
            &id,
            &id,
            DAY,
            800,
            &["/start", "/zzz"],
        ));
        let id = format!("p{i}");
        events.push(event(
            ch,
            "pageview",
            d,
            "/start",
            &id,
            &id,
            at(DAY_BEFORE, "23:59:00"),
        ));
        events.push(event(
            ch,
            "pageview",
            d,
            "/late",
            &id,
            &id,
            at(DAY, "00:01:00"),
        ));
    }
    insert(&events, &[]).await;

    Fixture {
        client,
        org_id,
        site_id,
        ch_id: ch,
        domain,
        signup,
        thanks,
    }
}

impl Fixture {
    fn path(&self) -> String {
        format!(
            "/api/org/{}/site/{}/journey/stats",
            self.org_id, self.site_id
        )
    }

    async fn journey(&self, body: Value) -> (u16, Value) {
        let resp = self.client.post(&self.path(), &body).await;
        let status = resp.status().as_u16();
        (status, resp.json().await.unwrap_or_default())
    }

    async fn ok(&self, extra: Value) -> Value {
        let mut body = json!({ "date_from": DAY, "date_to": DAY });
        for (k, v) in extra.as_object().unwrap() {
            body[k] = v.clone();
        }
        let (status, answer) = self.journey(body.clone()).await;
        assert_eq!(status, 200, "{body} -> {answer}");
        answer
    }
}

/// Every node of a response as the labels of its path from the anchor, with
/// its visits, sorted. A node's parent is the other end of its link to the
/// level below it.
fn paths(answer: &Value) -> Vec<(Vec<String>, u64)> {
    let nodes = answer["nodes"].as_array().expect("nodes");
    let mut parent = std::collections::HashMap::new();
    for l in answer["links"].as_array().expect("links") {
        let (s, t) = (
            l["source"].as_u64().unwrap() as usize,
            l["target"].as_u64().unwrap() as usize,
        );
        let (p, c) = if nodes[s]["level"].as_u64() < nodes[t]["level"].as_u64() {
            (s, t)
        } else {
            (t, s)
        };
        parent.insert(c, p);
    }
    let name = |n: &Value| match n["kind"].as_str().unwrap() {
        "anchor" | "page" => n["label"].as_str().unwrap().to_string(),
        kind => format!("({kind})"),
    };
    let mut out: Vec<(Vec<String>, u64)> = nodes
        .iter()
        .enumerate()
        .map(|(id, n)| {
            let mut path = vec![name(n)];
            let mut at = id;
            while let Some(&p) = parent.get(&at) {
                path.push(name(&nodes[p]));
                at = p;
            }
            path.reverse();
            (path, n["visits"].as_u64().unwrap())
        })
        .collect();
    out.sort();
    out
}

fn p(labels: &[&str], visits: u64) -> (Vec<String>, u64) {
    (labels.iter().map(|l| l.to_string()).collect(), visits)
}

/// Every shown node was followed by at least 5 distinct visitors; "other"
/// carries no visitor count at all.
fn assert_k_anonymous(answer: &Value) {
    for n in answer["nodes"].as_array().unwrap() {
        if n["kind"] == "other" {
            assert!(n["visitors"].is_null(), "{n}");
        } else {
            assert!(n["visitors"].as_u64().unwrap() >= MIN_VISITORS, "{n}");
        }
    }
}

#[tokio::test]
async fn test_next_pages_from_an_entry_page() {
    let f = fixture("journeynext").await;
    let answer = f.ok(json!({ "entry_page": "/start" })).await;
    assert_eq!(
        paths(&answer),
        vec![
            p(&["/start"], 25),
            p(&["/start", "(exit)"], 5),
            p(&["/start", "(other)"], 9),
            p(&["/start", "/a"], 11),
            p(&["/start", "/a", "/b"], 6),
            p(&["/start", "/a", "/b", "(exit)"], 6),
            p(&["/start", "/a", "/e"], 5),
            p(&["/start", "/a", "/e", "/f"], 5),
        ],
        "{answer}"
    );
    assert_k_anonymous(&answer);
    assert_eq!(
        answer["anchor"],
        json!({ "type": "entry_page", "label": "/start" })
    );
    assert_eq!(answer["nodes"][0]["visitors"], 22);
    assert_eq!(answer["meta"]["min_visitors"], 5);
    assert_eq!(answer["meta"]["depth"], 3);
    // Going forward, every flow runs from a node to the next page.
    for l in answer["links"].as_array().unwrap() {
        let (s, t) = (l["source"].as_u64().unwrap(), l["target"].as_u64().unwrap());
        assert!(
            answer["nodes"][s as usize]["level"].as_u64()
                < answer["nodes"][t as usize]["level"].as_u64(),
            "{l}"
        );
    }
}

#[tokio::test]
async fn test_paths_into_a_goal() {
    let f = fixture("journeyinto").await;
    let answer = f.ok(json!({ "goal_id": f.signup })).await;
    // /dashboard came after the signup, /after after G4's: neither is a
    // page before it. The 5 visits without a signup are not here.
    assert_eq!(
        paths(&answer),
        vec![
            p(&["Signup"], 15),
            p(&["Signup", "(other)"], 4),
            p(&["Signup", "/blog"], 5),
            p(&["Signup", "/blog", "(entrance)"], 5),
            p(&["Signup", "/register"], 6),
            p(&["Signup", "/register", "/pricing"], 6),
            p(&["Signup", "/register", "/pricing", "(entrance)"], 6),
        ],
        "{answer}"
    );
    assert_k_anonymous(&answer);
    assert_eq!(
        answer["anchor"],
        json!({ "type": "goal", "label": "Signup" })
    );
    // Into a goal, every flow runs towards it.
    for l in answer["links"].as_array().unwrap() {
        let (s, t) = (l["source"].as_u64().unwrap(), l["target"].as_u64().unwrap());
        assert!(
            answer["nodes"][s as usize]["level"].as_u64()
                > answer["nodes"][t as usize]["level"].as_u64(),
            "{l}"
        );
    }

    // A pageview goal's own pageview is not a page before it.
    let thanks = f.ok(json!({ "goal_id": f.thanks })).await;
    assert_eq!(
        paths(&thanks),
        vec![
            p(&["Thanks"], 5),
            p(&["Thanks", "/a"], 5),
            p(&["Thanks", "/a", "(entrance)"], 5),
        ],
        "{thanks}"
    );
}

#[tokio::test]
async fn test_a_path_seen_by_fewer_than_5_visitors_never_leaves_the_query() {
    let f = fixture("journeykanon").await;
    let site = seed::site(&f.site_id).await;
    let scope = SiteScope::for_site(&site);
    let service = QueryService::new(seed::clickhouse());
    let query = |extra: Value| {
        let mut body = json!({ "date_from": DAY, "date_to": DAY });
        for (k, v) in extra.as_object().unwrap() {
            body[k] = v.clone();
        }
        let request: JourneyRequest = serde_json::from_value(body).unwrap();
        JourneyQuery::parse(request, &[]).unwrap()
    };

    let rows = service
        .journey_paths(&scope, &query(json!({ "entry_page": "/start" })))
        .await
        .unwrap();
    let shown: Vec<Vec<String>> = rows
        .iter()
        .map(|r| r.prefix.iter().map(|(_, p)| p.clone()).collect())
        .collect();
    assert!(
        rows.iter().all(|r| r.visitors >= 5),
        "every row the query returns has 5 visitors or more: {rows:?}"
    );
    let has = |path: &[&str]| shown.iter().any(|s| s == path);
    assert!(!has(&["/c"]), "4 visitors: {shown:?}");
    assert!(!has(&["/d"]), "5 visits by 2 visitors: {shown:?}");
    assert!(has(&["/a", "/e"]), "exactly 5 visitors: {shown:?}");
    assert!(
        !has(&["/zzz"]) && !has(&["/late"]),
        "not the site's, or the day before"
    );

    // A fifth visitor down /c makes it a path.
    insert(
        &visit(f.ch_id, &f.domain, "b4", "b4", DAY, 900, &["/start", "/c"]),
        &[],
    )
    .await;
    let rows = service
        .journey_paths(&scope, &query(json!({ "entry_page": "/start" })))
        .await
        .unwrap();
    assert!(
        rows.iter()
            .any(|r| r.prefix.len() == 1 && r.prefix[0].1 == "/c" && r.visitors == 5),
        "{rows:?}"
    );
    let answer = f.ok(json!({ "entry_page": "/start" })).await;
    assert!(
        paths(&answer).contains(&p(&["/start", "/c"], 5)),
        "{answer}"
    );
    assert!(
        paths(&answer).contains(&p(&["/start", "(other)"], 5)),
        "only /d's visits are left: {answer}"
    );

    // An anchor with fewer than 5 visitors has no tree at all.
    let rows = service
        .journey_paths(
            &scope,
            &query(json!({ "entry_page": "/other-entry-that-few-used" })),
        )
        .await
        .unwrap();
    assert!(rows.is_empty(), "{rows:?}");
    insert(
        &visit(f.ch_id, &f.domain, "r1", "r1", DAY, 950, &["/rare", "/a"]),
        &[],
    )
    .await;
    let answer = f.ok(json!({ "entry_page": "/rare" })).await;
    assert_eq!(answer["nodes"], json!([]), "{answer}");
    assert_eq!(answer["links"], json!([]), "{answer}");
}

#[tokio::test]
async fn test_width_and_depth_bound_the_tree() {
    let f = fixture("journeybound").await;
    let narrow = f.ok(json!({ "entry_page": "/start", "width": 1 })).await;
    assert_eq!(
        paths(&narrow),
        vec![
            p(&["/start"], 25),
            p(&["/start", "(exit)"], 5),
            p(&["/start", "(other)"], 9),
            p(&["/start", "/a"], 11),
            p(&["/start", "/a", "(other)"], 5),
            p(&["/start", "/a", "/b"], 6),
            p(&["/start", "/a", "/b", "(exit)"], 6),
        ],
        "one page per level; /e's branch folds into other: {narrow}"
    );
    let shallow = f.ok(json!({ "entry_page": "/start", "depth": 1 })).await;
    assert_eq!(
        paths(&shallow),
        vec![
            p(&["/start"], 25),
            p(&["/start", "(exit)"], 5),
            p(&["/start", "(other)"], 9),
            p(&["/start", "/a"], 11),
        ],
        "{shallow}"
    );
}

#[tokio::test]
async fn test_journey_requests_are_checked() {
    let f = fixture("journeycheck").await;
    for (body, reason) in [
        (json!({ "date_from": DAY, "date_to": DAY }), "exactly one"),
        (
            json!({ "date_from": DAY, "date_to": DAY, "entry_page": "/start", "goal_id": f.signup }),
            "exactly one",
        ),
        (
            json!({ "date_from": DAY, "date_to": DAY, "goal_id": "000000000000000000000000" }),
            "not a goal of this site",
        ),
        (
            json!({ "date_from": DAY, "date_to": DAY, "entry_page": "/start", "depth": 6 }),
            "depth",
        ),
        (
            json!({ "date_from": DAY, "date_to": DAY, "entry_page": "/start", "width": 11 }),
            "width",
        ),
        (
            json!({ "date_from": DAY, "date_to": DAY_BEFORE, "entry_page": "/start" }),
            "before",
        ),
        (
            json!({ "date_from": DAY, "date_to": DAY, "entry_page": "/start", "min_visitors": 1 }),
            "invalid request body",
        ),
    ] {
        let (status, answer) = f.journey(body.clone()).await;
        assert_eq!(status, 400, "{body} -> {answer}");
        assert!(
            answer["message"].as_str().unwrap().contains(reason),
            "{body} -> {answer}"
        );
    }

    // In the stats rate-limit group.
    let settings = seed::settings();
    if settings.rate_limit.enabled {
        let resp = f
            .client
            .post(
                &f.path(),
                &json!({ "date_from": DAY, "date_to": DAY, "entry_page": "/start" }),
            )
            .await;
        let limit: u32 = resp.headers()["x-ratelimit-limit"]
            .to_str()
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(limit, settings.rate_limit.stats_rpm);
    }
}

#[tokio::test]
async fn test_journeys_across_orgs_are_not_found() {
    let victim = fixture("journeyvictim").await;
    let (intruder, intruder_org, intruder_site, _) = user_with_site("journeyintruder").await;
    let body = json!({ "date_from": DAY, "date_to": DAY, "entry_page": "/start" });
    let foreign = format!(
        "/api/org/{intruder_org}/site/{}/journey/stats",
        victim.site_id
    );
    assert_eq!(intruder.post(&foreign, &body).await.status(), 404);
    // The victim's goal is not a goal of the intruder's site.
    let resp = intruder
        .post(
            &format!("/api/org/{intruder_org}/site/{intruder_site}/journey/stats"),
            &json!({ "date_from": DAY, "date_to": DAY, "goal_id": victim.signup }),
        )
        .await;
    assert_eq!(resp.status(), 400);
    // And the victim's own org without membership.
    let victim_path = victim.path();
    assert_eq!(intruder.post(&victim_path, &body).await.status(), 403);
}
