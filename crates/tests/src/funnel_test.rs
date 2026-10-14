//! PS4: funnels. The definitions under a site (CRUD, the rules goals follow,
//! nothing across orgs), and the report against a fixed fixture of visits in
//! ClickHouse, which pins how `windowFunnel` reads a visit.

use serde_json::{Value, json};

use crate::helpers::{TestClient, user_with_site};
use crate::seed::{self, EventExt, at, ch_id, event, insert};

/// The day the fixture's visits are on.
const DAY: &str = "2026-04-14";
const DAY_BEFORE: &str = "2026-04-13";

/// A site with one goal (`signup`, a custom event) and a day of visits.
///
/// Funnel A is `/blog/*` -> `/pricing` -> goal `signup`:
/// - s1 (v1, linkedin): /blog/a, an engagement report, /features, /pricing,
///   another report, /register, signup. Other events come between every two
///   steps: reaches 3.
/// - s2 (v2, linkedin, from a Google search): /blog/b, /pricing: 2.
/// - s3 (v3, twitter): /pricing, /blog/c, signup. /pricing came first: 1.
/// - s4 (v4): /blog/a: 1.
/// - s5 (v5): /, /pricing, signup. Never on a blog page: 0.
/// - s6 (v1 again, no campaign): /blog/x: 1, and v1 is still one visitor.
/// - s7 (v7): started at 23:59 the day before (/blog/a), then /pricing and
///   signup after midnight: a visit of the day before.
/// - s8 (v8): every step, on localhost: not the site's.
///
/// So on DAY: 4, 2 and 1 visitors; with the day before, 5, 3 and 2. The
/// visits below touch none of funnel A's pages.
///
/// Funnel B is `/docs/*` -> `/docs/deep*` -> `/download`; a deep page
/// matches the first two steps:
/// - s10 (v10): /docs/deep-dive, /download: one pageview is one step, so 1.
/// - s11 (v11): /docs/a, /docs/deep-dive, /download: 3.
/// - s12 (v12): /docs/deep-one, /docs/deep-two: 2.
///
/// So on DAY: 3, 2 and 1.
///
/// Funnel C is `/sale/50%*` -> `/a_c*`, where `%` and `_` are literal:
/// - s20 (v20): /sale/50x-off, /abc/x: 0.
/// - s21 (v21): /sale/50%-off, /a_c/x: 2.
///
/// s13 (v13): /guide, then a `download` event.
struct Fixture {
    client: TestClient,
    org_id: String,
    site_id: String,
    signup_goal: String,
}

async fn fixture(tag: &str) -> Fixture {
    let (client, org_id, site_id, domain) = user_with_site(tag).await;
    let ch = ch_id(&site_id).await;
    let resp = client
        .post(
            &format!("/api/org/{org_id}/site/{site_id}/goal"),
            &json!({ "goal_type": "custom_event", "name": "Signup", "event_name": "signup" }),
        )
        .await;
    assert_eq!(resp.status(), 201, "creating the goal");
    let signup_goal = resp.json::<Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    let d = domain.as_str();
    let t = |hms: &str| at(DAY, hms);
    let pv = |path: &str, visitor: &str, visit: &str, hms: &str| {
        event(ch, "pageview", d, path, visitor, visit, t(hms))
    };
    let named = |name: &str, path: &str, visitor: &str, visit: &str, hms: &str| {
        event(ch, name, d, path, visitor, visit, t(hms))
    };
    // s2 landed from a Google search: channel `search`; every other visit is
    // `direct`.
    let mut s2_landing = pv("/blog/b", "v2", "s2", "11:00:00").utm_source("linkedin");
    s2_landing.referrer = "https://www.google.com/".into();
    s2_landing.referrer_source = "Google".into();
    let events = vec![
        pv("/blog/a", "v1", "s1", "10:00:00").utm_source("linkedin"),
        named("engagement", "/blog/a", "v1", "s1", "10:00:30")
            .utm_source("linkedin")
            .props(&[("duration", "30"), ("pv", "p1")]),
        pv("/features", "v1", "s1", "10:01:00"),
        pv("/pricing", "v1", "s1", "10:02:00"),
        named("engagement", "/pricing", "v1", "s1", "10:02:30")
            .props(&[("duration", "30"), ("pv", "p2")]),
        pv("/register", "v1", "s1", "10:03:00"),
        named("signup", "/register", "v1", "s1", "10:03:10"),
        s2_landing,
        pv("/pricing", "v2", "s2", "11:01:00"),
        pv("/pricing", "v3", "s3", "12:00:00").utm_source("twitter"),
        pv("/blog/c", "v3", "s3", "12:01:00"),
        named("signup", "/blog/c", "v3", "s3", "12:02:00"),
        pv("/blog/a", "v4", "s4", "13:00:00"),
        pv("/", "v5", "s5", "14:00:00"),
        pv("/pricing", "v5", "s5", "14:01:00"),
        named("signup", "/pricing", "v5", "s5", "14:02:00"),
        pv("/blog/x", "v1", "s6", "15:00:00"),
        event(
            ch,
            "pageview",
            d,
            "/blog/a",
            "v7",
            "s7",
            at(DAY_BEFORE, "23:59:00"),
        ),
        pv("/pricing", "v7", "s7", "00:01:00"),
        named("signup", "/pricing", "v7", "s7", "00:02:00"),
        event(
            ch,
            "pageview",
            "localhost",
            "/blog/a",
            "v8",
            "s8",
            t("09:00:00"),
        ),
        event(
            ch,
            "pageview",
            "localhost",
            "/pricing",
            "v8",
            "s8",
            t("09:01:00"),
        ),
        event(
            ch,
            "signup",
            "localhost",
            "/pricing",
            "v8",
            "s8",
            t("09:02:00"),
        ),
        pv("/docs/deep-dive", "v10", "s10", "16:00:00"),
        pv("/download", "v10", "s10", "16:01:00"),
        pv("/docs/a", "v11", "s11", "16:30:00"),
        pv("/docs/deep-dive", "v11", "s11", "16:31:00"),
        pv("/download", "v11", "s11", "16:32:00"),
        pv("/docs/deep-one", "v12", "s12", "17:00:00"),
        pv("/docs/deep-two", "v12", "s12", "17:01:00"),
        pv("/guide", "v13", "s13", "18:00:00"),
        named("download", "/guide", "v13", "s13", "18:01:00"),
        pv("/sale/50x-off", "v20", "s20", "19:00:00"),
        pv("/abc/x", "v20", "s20", "19:01:00"),
        pv("/sale/50%-off", "v21", "s21", "19:30:00"),
        pv("/a_c/x", "v21", "s21", "19:31:00"),
    ];
    insert(&events, &[]).await;

    Fixture {
        client,
        org_id,
        site_id,
        signup_goal,
    }
}

impl Fixture {
    fn path(&self) -> String {
        format!("/api/org/{}/site/{}/funnel", self.org_id, self.site_id)
    }

    /// Creates a funnel; its id.
    async fn funnel(&self, name: &str, steps: Value) -> String {
        let resp = self
            .client
            .post(&self.path(), &json!({ "name": name, "steps": steps }))
            .await;
        assert_eq!(resp.status(), 201, "creating {name}");
        resp.json::<Value>().await.unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string()
    }

    async fn report(&self, funnel_id: &str, body: Value) -> (u16, Value) {
        let resp = self
            .client
            .post(&format!("{}/{funnel_id}/stats", self.path()), &body)
            .await;
        let status = resp.status().as_u16();
        (status, resp.json().await.unwrap_or_default())
    }

    async fn ok(&self, funnel_id: &str, body: Value) -> Value {
        let (status, answer) = self.report(funnel_id, body.clone()).await;
        assert_eq!(status, 200, "{body} -> {answer}");
        answer
    }

    /// Funnel A: blog, pricing, signup.
    async fn funnel_a(&self) -> String {
        self.funnel(
            "Blog to signup",
            json!([
                { "type": "page", "pattern": "/blog/*", "label": "Read a post" },
                { "type": "page", "pattern": "/pricing" },
                { "type": "goal", "goal_id": self.signup_goal },
            ]),
        )
        .await
    }
}

fn page(pattern: &str) -> Value {
    json!({ "type": "page", "pattern": pattern })
}

fn day() -> Value {
    json!({ "date_from": DAY, "date_to": DAY })
}

/// The visitors of each step.
fn visitors(steps: &Value) -> Vec<u64> {
    steps
        .as_array()
        .expect("steps")
        .iter()
        .map(|s| s["visitors"].as_u64().expect("visitors"))
        .collect()
}

#[tokio::test]
async fn test_a_funnel_counts_ordered_steps_within_a_visit() {
    let f = fixture("funnel").await;
    let id = f.funnel_a().await;
    let answer = f.ok(&id, day()).await;
    assert_eq!(
        answer["steps"],
        json!([
            { "label": "Read a post", "type": "page", "visitors": 4,
              "conversion_from_previous": null, "conversion_from_start": 1.0, "dropoff": null },
            { "label": "/pricing", "type": "page", "visitors": 2,
              "conversion_from_previous": 0.5, "conversion_from_start": 0.5, "dropoff": 2 },
            { "label": "Signup", "type": "goal", "visitors": 1,
              "conversion_from_previous": 0.5, "conversion_from_start": 0.25, "dropoff": 1 },
        ]),
        "{answer}"
    );
    assert_eq!(answer["breakdown"], Value::Null);
    assert_eq!(answer["funnel"]["id"], id.as_str());
    assert_eq!(answer["meta"]["date_from"], DAY);
    assert_eq!(answer["meta"]["hostnames"].as_array().unwrap().len(), 2);

    // A visit belongs to the day it started: the one from 23:59 the day
    // before counts with that day.
    let two_days = f
        .ok(&id, json!({ "date_from": DAY_BEFORE, "date_to": DAY }))
        .await;
    assert_eq!(visitors(&two_days["steps"]), [5, 3, 2], "{two_days}");
    let day_before = f
        .ok(
            &id,
            json!({ "date_from": DAY_BEFORE, "date_to": DAY_BEFORE }),
        )
        .await;
    assert_eq!(visitors(&day_before["steps"]), [1, 1, 1], "{day_before}");
}

#[tokio::test]
async fn test_one_event_serves_one_step_and_a_repeat_does_not_lose_one() {
    let f = fixture("funnelonce").await;
    let id = f
        .funnel(
            "Deep reads",
            json!([page("/docs/*"), page("/docs/deep*"), page("/download")]),
        )
        .await;
    let answer = f.ok(&id, day()).await;
    // ClickHouse's default mode would count s10's one deep pageview as two
    // steps (3, 3, 2); 'strict_increase' would lose s11 and s12 (3, 0, 0).
    assert_eq!(visitors(&answer["steps"]), [3, 2, 1], "{answer}");
}

#[tokio::test]
async fn test_a_star_is_a_wildcard_and_every_other_character_is_literal() {
    let f = fixture("funnellike").await;
    let id = f
        .funnel("Sale", json!([page("/sale/50%*"), page("/a_c*")]))
        .await;
    let answer = f.ok(&id, day()).await;
    assert_eq!(visitors(&answer["steps"]), [1, 1], "{answer}");

    let exact = f
        .funnel("Exact", json!([page("/blog/a"), page("/pricing")]))
        .await;
    let answer = f.ok(&exact, day()).await;
    // Exactly /blog/a: s1 and s4; then /pricing: s1. The /blog/a of s7 was
    // the day before, and the one of s8 on localhost.
    assert_eq!(visitors(&answer["steps"]), [2, 1], "{answer}");
}

#[tokio::test]
async fn test_a_funnel_by_campaign_and_filtered_by_one() {
    let f = fixture("funnelutm").await;
    let id = f.funnel_a().await;
    let mut body = day();
    body["breakdown"] = json!("utm_source");
    let answer = f.ok(&id, body.clone()).await;
    assert_eq!(
        visitors(&answer["steps"]),
        [4, 2, 1],
        "the totals stay: {answer}"
    );
    let rows: Vec<(String, Vec<u64>)> = answer["breakdown"]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["value"].as_str().unwrap().to_string(),
                visitors(&r["steps"]),
            )
        })
        .collect();
    // The campaign is the landing's: s1's signup came three pages after it.
    // v1 is under "" too, for its second visit. The source is normalised as
    // in every report: `twitter` is `x`.
    assert_eq!(
        rows,
        [
            (String::new(), vec![2, 0, 0]),
            ("linkedin".to_string(), vec![2, 2, 1]),
            ("x".to_string(), vec![1, 0, 0]),
        ],
        "{answer}"
    );
    assert_eq!(answer["breakdown"]["dimension"], "utm_source");
    assert_eq!(answer["breakdown"]["has_more"], false);

    body["limit"] = json!(2);
    let paged = f.ok(&id, body).await;
    assert_eq!(paged["breakdown"]["rows"].as_array().unwrap().len(), 2);
    assert_eq!(paged["breakdown"]["has_more"], true, "{paged}");

    let mut filtered = day();
    filtered["filters"] =
        json!([{ "dimension": "utm_source", "operator": "is", "value": "linkedin" }]);
    let answer = f.ok(&id, filtered).await;
    assert_eq!(visitors(&answer["steps"]), [2, 2, 1], "{answer}");

    // By channel, from the landing's referrer: s2 came from a search.
    let mut by_channel = day();
    by_channel["breakdown"] = json!("channel");
    let answer = f.ok(&id, by_channel).await;
    let rows: Vec<(String, Vec<u64>)> = answer["breakdown"]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["value"].as_str().unwrap().to_string(),
                visitors(&r["steps"]),
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            ("direct".to_string(), vec![3, 1, 1]),
            ("search".to_string(), vec![1, 1, 0]),
        ],
        "{answer}"
    );
}

#[tokio::test]
async fn test_a_deleted_goal_step_matches_nothing() {
    let f = fixture("funnelgoal").await;
    let resp = f
        .client
        .post(
            &format!("/api/org/{}/site/{}/goal", f.org_id, f.site_id),
            &json!({ "goal_type": "custom_event", "name": "Download", "event_name": "download" }),
        )
        .await;
    assert_eq!(resp.status(), 201);
    let download = resp.json::<Value>().await.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let id = f
        .funnel(
            "Downloads",
            json!([page("/guide"), { "type": "goal", "goal_id": download }]),
        )
        .await;
    let before = f.ok(&id, day()).await;
    assert_eq!(visitors(&before["steps"]), [1, 1], "{before}");
    assert_eq!(before["steps"][1]["label"], "Download");

    let resp = f
        .client
        .delete(&format!(
            "/api/org/{}/site/{}/goal/{download}",
            f.org_id, f.site_id
        ))
        .await;
    assert_eq!(resp.status(), 204);
    let after = f.ok(&id, day()).await;
    assert_eq!(visitors(&after["steps"]), [1, 0], "{after}");
    assert_eq!(after["steps"][1]["label"], "Deleted goal");
    assert_eq!(after["steps"][1]["goal_missing"], true);
    assert!(after["steps"][0].get("goal_missing").is_none(), "{after}");
}

#[tokio::test]
async fn test_funnel_crud_and_its_checks() {
    let f = fixture("funnelcrud").await;
    let id = f.funnel_a().await;

    let list: Value = f.client.get(&f.path()).await.json().await.unwrap();
    assert_eq!(list.as_array().unwrap().len(), 1, "{list}");
    assert_eq!(
        list[0],
        json!({
            "id": id, "site_id": f.site_id, "name": "Blog to signup",
            "steps": [
                { "type": "page", "pattern": "/blog/*", "goal_id": null, "label": "Read a post" },
                { "type": "page", "pattern": "/pricing", "goal_id": null, "label": null },
                { "type": "goal", "pattern": null, "goal_id": f.signup_goal, "label": null },
            ],
        })
    );

    let one = format!("{}/{id}", f.path());
    let resp = f
        .client
        .put(
            &one,
            &json!({ "name": "Renamed", "steps": [page("/"), page("/pricing")] }),
        )
        .await;
    assert_eq!(resp.status(), 200);
    let got: Value = f.client.get(&one).await.json().await.unwrap();
    assert_eq!(got["name"], "Renamed");
    assert_eq!(got["steps"].as_array().unwrap().len(), 2, "{got}");

    for (body, status, reason) in [
        (
            json!({ "name": "One step", "steps": [page("/")] }),
            422,
            "2 to 8 steps",
        ),
        (
            json!({ "name": "Nine", "steps": vec![page("/"); 9] }),
            422,
            "2 to 8 steps",
        ),
        (
            json!({ "name": "F", "steps": [page("/"), { "type": "goal", "goal_id": "000000000000000000000000" }] }),
            422,
            "goal_id is not a goal of this site",
        ),
        (
            json!({ "name": "F", "steps": [page("pricing"), page("/")] }),
            422,
            "starts with / or *",
        ),
        (
            json!({ "name": "F", "steps": [page("/"), page("/")], "site_id": "x" }),
            400,
            "invalid request body",
        ),
    ] {
        let resp = f.client.post(&f.path(), &body).await;
        assert_eq!(resp.status(), status, "{body}");
        let answer: Value = resp.json().await.unwrap();
        assert!(
            answer["message"].as_str().unwrap().contains(reason),
            "{body} -> {answer}"
        );
        let resp = f.client.put(&one, &body).await;
        assert_eq!(resp.status(), status, "update with {body}");
    }

    for (body, reason) in [
        (json!({ "date_from": DAY, "date_to": DAY_BEFORE }), "before"),
        (
            json!({ "date_from": DAY, "date_to": DAY, "breakdown": "country" }),
            "unknown breakdown",
        ),
        (json!({ "date_from": DAY }), "invalid request body"),
    ] {
        let (status, answer) = f.report(&id, body.clone()).await;
        assert_eq!(status, 400, "{body} -> {answer}");
        assert!(
            answer["message"].as_str().unwrap().contains(reason),
            "{body} -> {answer}"
        );
    }

    // The report is rate-limited with the stats, the definitions with the API.
    let settings = seed::settings();
    if settings.rate_limit.enabled {
        let limit = |resp: &reqwest::Response| -> u32 {
            resp.headers()["x-ratelimit-limit"]
                .to_str()
                .unwrap()
                .parse()
                .unwrap()
        };
        let resp = f.client.post(&format!("{one}/stats"), &day()).await;
        assert_eq!(limit(&resp), settings.rate_limit.stats_rpm);
        assert_eq!(
            limit(&f.client.get(&f.path()).await),
            settings.rate_limit.api_rpm
        );
    }

    assert_eq!(f.client.delete(&one).await.status(), 204);
    assert_eq!(f.client.get(&one).await.status(), 404);
    assert_eq!(f.client.delete(&one).await.status(), 404);
    assert_eq!(f.report(&id, day()).await.0, 404);
}

#[tokio::test]
async fn test_a_viewer_reads_funnels_and_only_an_admin_writes_them() {
    let f = fixture("funnelrole").await;
    let id = f.funnel_a().await;
    let resp = f
        .client
        .post(
            &format!("/api/org/{}/invite", f.org_id),
            &json!({ "role": "viewer", "max_uses": 1, "expires_in_hours": 24 }),
        )
        .await;
    assert_eq!(resp.status(), 201);
    let code = resp.json::<Value>().await.unwrap()["code"]
        .as_str()
        .unwrap()
        .to_string();
    let mut viewer = TestClient::new();
    let uid = uuid::Uuid::new_v4().to_string();
    viewer
        .register(
            &format!("viewer-{uid}@purestat.test"),
            &format!("viewer-{}", &uid[..8]),
            "TestPass123!",
        )
        .await;
    let resp = viewer
        .post(&format!("/api/invite/{code}/accept"), &json!({}))
        .await;
    assert_eq!(resp.status(), 200, "the viewer joins");

    let one = format!("{}/{id}", f.path());
    assert_eq!(viewer.get(&f.path()).await.status(), 200);
    assert_eq!(viewer.get(&one).await.status(), 200);
    assert_eq!(
        viewer.post(&format!("{one}/stats"), &day()).await.status(),
        200
    );
    let body = json!({ "name": "Viewer's", "steps": [page("/"), page("/pricing")] });
    assert_eq!(viewer.post(&f.path(), &body).await.status(), 403);
    assert_eq!(viewer.put(&one, &body).await.status(), 403);
    assert_eq!(viewer.delete(&one).await.status(), 403);
    assert_eq!(f.client.get(&one).await.status(), 200, "still there");
}

#[tokio::test]
async fn test_funnels_across_orgs_are_not_found() {
    let victim = fixture("funnelvictim").await;
    let id = victim.funnel_a().await;
    let (intruder, intruder_org, intruder_site, _) = user_with_site("funnelintruder").await;
    let body = json!({ "name": "Mine", "steps": [page("/"), page("/pricing")] });

    // The victim's site under the intruder's org.
    let foreign = format!("/api/org/{intruder_org}/site/{}/funnel", victim.site_id);
    assert_eq!(intruder.get(&foreign).await.status(), 404, "list");
    assert_eq!(intruder.post(&foreign, &body).await.status(), 404, "create");
    let foreign_one = format!("{foreign}/{id}");
    assert_eq!(intruder.get(&foreign_one).await.status(), 404, "get");
    assert_eq!(
        intruder.put(&foreign_one, &body).await.status(),
        404,
        "update"
    );
    assert_eq!(intruder.delete(&foreign_one).await.status(), 404, "delete");
    assert_eq!(
        intruder
            .post(&format!("{foreign_one}/stats"), &day())
            .await
            .status(),
        404,
        "report"
    );
    // The victim's funnel id under the intruder's own site.
    let own_one = format!("/api/org/{intruder_org}/site/{intruder_site}/funnel/{id}");
    assert_eq!(intruder.get(&own_one).await.status(), 404);
    assert_eq!(intruder.put(&own_one, &body).await.status(), 404);
    assert_eq!(intruder.delete(&own_one).await.status(), 404);
    assert_eq!(
        intruder
            .post(&format!("{own_one}/stats"), &day())
            .await
            .status(),
        404
    );
    // A goal of the victim's site is not a goal of the intruder's.
    let resp = intruder
        .post(
            &format!("/api/org/{intruder_org}/site/{intruder_site}/funnel"),
            &json!({ "name": "F", "steps": [page("/"), { "type": "goal", "goal_id": victim.signup_goal }] }),
        )
        .await;
    assert_eq!(resp.status(), 422);

    let got: Value = victim
        .client
        .get(&format!("{}/{id}", victim.path()))
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(got["name"], "Blog to signup", "unchanged: {got}");
}

#[tokio::test]
async fn test_deleting_a_site_deletes_its_funnels() {
    let f = fixture("funnelsite").await;
    f.funnel_a().await;
    let resp = f
        .client
        .delete(&format!("/api/org/{}/site/{}", f.org_id, f.site_id))
        .await;
    assert_eq!(resp.status(), 204);
    let db = purestat_db::connect(&seed::settings()).await.unwrap();
    let left = db
        .collection::<bson::Document>("funnels")
        .count_documents(
            bson::doc! { "site_id": bson::oid::ObjectId::parse_str(&f.site_id).unwrap() },
        )
        .await
        .unwrap();
    assert_eq!(left, 0);
}
