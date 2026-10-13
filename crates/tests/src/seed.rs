//! Rows written straight to ClickHouse under a site's id, for tests that need
//! exact numbers: ingest stamps everything with the current time and derives
//! its own visitors and visits, so it cannot build a fixed fixture.

use purestat_db::clickhouse::schemas::{Event, Session};
use purestat_db::models::Site;

use crate::helpers::TestClient;

pub fn settings() -> purestat_config::Settings {
    purestat_config::Settings::load().expect("PURESTAT__* settings")
}

pub fn clickhouse() -> clickhouse::Client {
    purestat_db::clickhouse::connection::connect(&settings())
}

/// The site document as stored, read the way the API reads it.
pub async fn site(site_id: &str) -> Site {
    let db = purestat_db::connect(&settings()).await.expect("mongo");
    let oid = bson::oid::ObjectId::parse_str(site_id).expect("a site id");
    db.collection::<Site>(Site::COLLECTION)
        .find_one(bson::doc! { "_id": oid })
        .await
        .expect("find site")
        .expect("the site exists")
}

/// The id the site's rows carry in ClickHouse.
pub async fn ch_id(site_id: &str) -> u64 {
    site(site_id).await.clickhouse_id()
}

/// A UTC moment, as ClickHouse stores it.
pub fn at(date: &str, hms: &str) -> time::OffsetDateTime {
    let t = chrono::NaiveDateTime::parse_from_str(&format!("{date} {hms}"), "%Y-%m-%d %H:%M:%S")
        .expect("a date and a time")
        .and_utc();
    time::OffsetDateTime::from_unix_timestamp(t.timestamp()).expect("a timestamp")
}

/// An event with nothing set but what identifies it; the builder methods add
/// the rest.
pub fn event(
    ch_id: u64,
    name: &str,
    host: &str,
    path: &str,
    visitor: &str,
    session: &str,
    ts: time::OffsetDateTime,
) -> Event {
    Event {
        site_id: ch_id,
        visitor_hash: visitor.into(),
        session_id: session.into(),
        event_name: name.into(),
        url: format!("https://{host}{path}"),
        path: path.into(),
        hostname: host.into(),
        referrer: String::new(),
        referrer_source: "Direct".into(),
        utm_source: String::new(),
        utm_medium: String::new(),
        utm_campaign: String::new(),
        utm_content: String::new(),
        utm_term: String::new(),
        country: "DE".into(),
        region: String::new(),
        city: String::new(),
        browser: "Firefox".into(),
        browser_version: String::new(),
        os: "Linux".into(),
        os_version: String::new(),
        device_type: "desktop".into(),
        screen_width: 1280,
        screen_height: 0,
        prop_keys: vec![],
        prop_values: vec![],
        revenue_amount: None,
        revenue_currency: None,
        timestamp: ts,
    }
}

pub trait EventExt {
    fn utm_source(self, source: &str) -> Self;
    fn props(self, props: &[(&str, &str)]) -> Self;
}

impl EventExt for Event {
    fn utm_source(mut self, source: &str) -> Self {
        self.utm_source = source.into();
        self
    }

    fn props(mut self, props: &[(&str, &str)]) -> Self {
        self.prop_keys = props.iter().map(|(k, _)| k.to_string()).collect();
        self.prop_values = props.iter().map(|(_, v)| v.to_string()).collect();
        self
    }
}

/// A visit as the session sweeper writes it.
#[allow(clippy::too_many_arguments)]
pub fn session(
    ch_id: u64,
    visitor: &str,
    session: &str,
    entry: &str,
    exit: &str,
    pageviews: u32,
    duration: u32,
    utm_source: &str,
    started: time::OffsetDateTime,
) -> Session {
    Session {
        site_id: ch_id,
        visitor_hash: visitor.into(),
        session_id: session.into(),
        is_bounce: u8::from(pageviews <= 1),
        entry_page: entry.into(),
        exit_page: exit.into(),
        pageviews,
        events_count: pageviews,
        duration,
        referrer: String::new(),
        referrer_source: "Direct".into(),
        utm_source: utm_source.into(),
        utm_medium: String::new(),
        utm_campaign: String::new(),
        utm_content: String::new(),
        utm_term: String::new(),
        country: "DE".into(),
        browser: "Firefox".into(),
        os: "Linux".into(),
        device_type: "desktop".into(),
        started_at: started,
        ended_at: started + time::Duration::seconds(i64::from(duration)),
    }
}

pub async fn insert(events: &[Event], sessions: &[Session]) {
    let ch = clickhouse();
    if !events.is_empty() {
        let mut insert = ch.insert("events").expect("insert events");
        for e in events {
            insert.write(e).await.expect("write event");
        }
        insert.end().await.expect("events written");
    }
    if !sessions.is_empty() {
        let mut insert = ch.insert("sessions").expect("insert sessions");
        for s in sessions {
            insert.write(s).await.expect("write session");
        }
        insert.end().await.expect("sessions written");
    }
}

/// Mints an API key for the site through the API, and returns it.
pub async fn api_key(
    client: &TestClient,
    org_id: &str,
    site_id: &str,
    scopes: &[&str],
) -> (String, String) {
    let resp = client
        .post(
            &format!("/api/org/{org_id}/site/{site_id}/api-key"),
            &serde_json::json!({ "name": "test key", "scopes": scopes }),
        )
        .await;
    assert_eq!(resp.status(), 201, "minting an API key");
    let key: serde_json::Value = resp.json().await.unwrap();
    (
        key["key"].as_str().unwrap().to_string(),
        key["id"].as_str().unwrap().to_string(),
    )
}

/// Creates a goal through the API.
pub async fn goal(client: &TestClient, org_id: &str, site_id: &str, body: serde_json::Value) {
    let resp = client
        .post(&format!("/api/org/{org_id}/site/{site_id}/goal"), &body)
        .await;
    assert!(
        resp.status().is_success(),
        "creating a goal: {}",
        resp.status()
    );
}

/// POST /api/v1/query with an optional X-API-Key; the status and the body.
pub async fn v1(key: Option<&str>, body: &serde_json::Value) -> (u16, serde_json::Value) {
    let mut req = reqwest::Client::new()
        .post(format!("{}/api/v1/query", crate::helpers::base_url()))
        .json(body);
    if let Some(key) = key {
        req = req.header("x-api-key", key);
    }
    let resp = req.send().await.expect("Request failed");
    let status = resp.status().as_u16();
    (status, resp.json().await.unwrap_or_default())
}
