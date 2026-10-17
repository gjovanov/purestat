//! Deleting a site deletes its rows in ClickHouse (roomler-ai FR-102 PS0b):
//! every row of a site with an id of its own, only its own rows when its id
//! is shared with another live site, and the visits still live in Redis.

use std::time::Duration;

use bson::oid::ObjectId;
use purestat_db::models::Site;

use crate::helpers::{base_url, user_with_site};
use crate::seed::{self, at, ch_id, event, insert, session};

const DAY: &str = "2026-03-11";

/// `(events, sessions)` carrying the id.
async fn rows(ch_id: u64) -> (u64, u64) {
    let ch = seed::clickhouse();
    let events = ch
        .query("SELECT count() FROM events WHERE site_id = ?")
        .bind(ch_id)
        .fetch_one::<u64>()
        .await
        .expect("count events");
    let sessions = ch
        .query("SELECT count() FROM sessions WHERE site_id = ?")
        .bind(ch_id)
        .fetch_one::<u64>()
        .await
        .expect("count sessions");
    (events, sessions)
}

/// Waits for the background mutations until `count` returns `expected`.
async fn settles<F, Fut, T>(expected: T, count: F) -> T
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = T>,
    T: PartialEq + std::fmt::Debug,
{
    let mut last = count().await;
    for _ in 0..60 {
        if last == expected {
            return last;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
        last = count().await;
    }
    last
}

/// The hostnames stored under the id, sorted, one entry per event.
async fn hostnames(ch_id: u64) -> Vec<String> {
    seed::clickhouse()
        .query("SELECT hostname FROM events WHERE site_id = ? ORDER BY hostname, path")
        .bind(ch_id)
        .fetch_all::<String>()
        .await
        .expect("read hostnames")
}

/// The visits stored under the id, sorted.
async fn visits(ch_id: u64) -> Vec<String> {
    seed::clickhouse()
        .query("SELECT session_id FROM sessions WHERE site_id = ? ORDER BY session_id")
        .bind(ch_id)
        .fetch_all::<String>()
        .await
        .expect("read sessions")
}

#[tokio::test]
async fn test_deleting_a_site_deletes_its_rows_in_clickhouse() {
    let (client, org_id, site_id, domain) = user_with_site("delete").await;
    let ch = ch_id(&site_id).await;
    // Another site, whose rows must stay. (The free plan has one site per
    // org, so it is another user's.)
    let (_other_client, _other_org, other_id, other_domain) = user_with_site("delete-other").await;
    let other_ch = ch_id(&other_id).await;

    let t = |hms: &str| at(DAY, hms);
    insert(
        &[
            event(ch, "pageview", &domain, "/a", "v1", "s1", t("10:00:00")),
            event(ch, "pageview", &domain, "/b", "v1", "s1", t("10:01:00")),
            // No read counts it, and it is still the site's tracker's.
            event(
                ch,
                "pageview",
                "localhost",
                "/dev",
                "v2",
                "s2",
                t("10:02:00"),
            ),
            event(
                other_ch,
                "pageview",
                &other_domain,
                "/c",
                "v3",
                "s3",
                t("10:03:00"),
            ),
        ],
        &[
            session(ch, "v1", "s1", "/a", "/b", 2, 60, "", t("10:00:00")),
            session(ch, "v2", "s2", "/dev", "/dev", 1, 0, "", t("10:02:00")),
            session(other_ch, "v3", "s3", "/c", "/c", 1, 0, "", t("10:03:00")),
        ],
    )
    .await;
    assert_eq!(rows(ch).await, (3, 2));
    assert_eq!(rows(other_ch).await, (1, 1));

    let resp = client
        .delete(&format!("/api/org/{org_id}/site/{site_id}"))
        .await;
    assert_eq!(resp.status(), 204);

    assert_eq!(
        settles((0, 0), || rows(ch)).await,
        (0, 0),
        "every row of the deleted site is gone"
    );
    assert_eq!(rows(other_ch).await, (1, 1), "the other site's rows stay");
    let resp = client
        .get(&format!("/api/org/{org_id}/site/{site_id}"))
        .await;
    assert_eq!(resp.status(), 404, "the site is gone");
}

#[tokio::test]
async fn test_deleting_a_site_that_shares_its_id_keeps_the_other_sites_rows() {
    let (client, org_id, _site_id, _domain) = user_with_site("sharedid").await;
    let uid = uuid::Uuid::new_v4().simple().to_string();

    // Two sites from before ch_id existed, created in the same second by one
    // process: their ObjectIds share the 8 bytes their ClickHouse id is.
    let mut prefix = [0u8; 8];
    prefix.copy_from_slice(&uuid::Uuid::new_v4().as_bytes()[..8]);
    let oid = |last: u8| {
        let mut bytes = [0u8; 12];
        bytes[..8].copy_from_slice(&prefix);
        bytes[11] = last;
        ObjectId::from_bytes(bytes)
    };
    let (a_oid, b_oid) = (oid(1), oid(2));
    let a = format!("a-{}.example.com", &uid[..8]);
    let b = format!("b-{}.example.com", &uid[..8]);
    let db = purestat_db::connect(&seed::settings())
        .await
        .expect("mongo");
    let org_oid = ObjectId::parse_str(&org_id).unwrap();
    for (id, domain) in [(a_oid, &a), (b_oid, &b)] {
        db.collection::<bson::Document>(Site::COLLECTION)
            .insert_one(bson::doc! {
                "_id": id,
                "org_id": org_oid,
                "domain": domain,
                "name": domain,
                "timezone": "UTC",
                "is_public": false,
                "allowed_hostnames": [],
                "created_at": bson::DateTime::now(),
                "updated_at": bson::DateTime::now(),
            })
            .await
            .expect("insert a legacy site");
    }
    let shared = Site::legacy_clickhouse_id(&a_oid);
    assert_eq!(shared, Site::legacy_clickhouse_id(&b_oid));

    let www_a = format!("www.{a}");
    let t = |hms: &str| at(DAY, hms);
    insert(
        &[
            event(shared, "pageview", &a, "/a1", "va1", "sa1", t("09:00:00")),
            event(
                shared,
                "pageview",
                &www_a,
                "/a2",
                "va2",
                "sa2",
                t("09:01:00"),
            ),
            event(shared, "pageview", &b, "/b1", "vb1", "sb1", t("09:02:00")),
            // One visit on both sites: B reads it, so it stays.
            event(shared, "pageview", &a, "/m1", "vm", "sm", t("09:03:00")),
            event(shared, "pageview", &b, "/m2", "vm", "sm", t("09:04:00")),
            // On a host no site counts: no one's rows to delete.
            event(
                shared,
                "pageview",
                "localhost",
                "/dev",
                "vj",
                "sj",
                t("09:05:00"),
            ),
        ],
        &[
            session(shared, "va1", "sa1", "/a1", "/a1", 1, 0, "", t("09:00:00")),
            session(shared, "va2", "sa2", "/a2", "/a2", 1, 0, "", t("09:01:00")),
            session(shared, "vb1", "sb1", "/b1", "/b1", 1, 0, "", t("09:02:00")),
            session(shared, "vm", "sm", "/m1", "/m2", 2, 60, "", t("09:03:00")),
            session(shared, "vj", "sj", "/dev", "/dev", 1, 0, "", t("09:05:00")),
        ],
    )
    .await;

    let resp = client
        .delete(&format!("/api/org/{org_id}/site/{}", a_oid.to_hex()))
        .await;
    assert_eq!(resp.status(), 204);

    let mut expected_hosts = vec![b.clone(), b.clone(), "localhost".to_string()];
    expected_hosts.sort();
    assert_eq!(
        settles(expected_hosts.clone(), || hostnames(shared)).await,
        expected_hosts,
        "A's events are gone; B's, and the ones no site counts, stay"
    );
    assert_eq!(
        visits(shared).await,
        ["sb1", "sj", "sm"],
        "A's own visits are gone; the shared visit and the uncounted one stay"
    );
    let resp = client
        .get(&format!("/api/org/{org_id}/site/{}", b_oid.to_hex()))
        .await;
    assert_eq!(resp.status(), 200, "B is untouched");
}

#[tokio::test]
async fn test_a_deleted_sites_live_visit_never_reaches_clickhouse() {
    let (client, org_id, site_id, domain) = user_with_site("livevisit").await;
    let ch = ch_id(&site_id).await;

    let resp = reqwest::Client::new()
        .post(format!("{}/api/event", base_url()))
        .header(
            "user-agent",
            "Mozilla/5.0 (X11; Linux x86_64; rv:131.0) Gecko/20100101 Firefox/131.0",
        )
        .header("x-forwarded-for", "203.0.113.77")
        .json(&serde_json::json!({
            "domain": domain,
            "name": "pageview",
            "url": format!("https://{domain}/live"),
            "referrer": "",
        }))
        .send()
        .await
        .expect("Request failed");
    assert_eq!(resp.status(), 202);

    let settings = seed::settings();
    let mut redis = redis::aio::ConnectionManager::new(
        redis::Client::open(settings.redis.url.as_str()).expect("a Redis URL"),
    )
    .await
    .expect("redis");
    let lookups = |redis: redis::aio::ConnectionManager| async move {
        let mut redis = redis;
        let keys: Vec<String> = redis::cmd("KEYS")
            .arg(format!("purestat:vs:{ch}:*"))
            .query_async(&mut redis)
            .await
            .expect("KEYS");
        keys.len()
    };
    assert_eq!(
        lookups(redis.clone()).await,
        1,
        "the visit is live in Redis"
    );

    let resp = client
        .delete(&format!("/api/org/{org_id}/site/{site_id}"))
        .await;
    assert_eq!(resp.status(), 204);

    assert_eq!(
        lookups(redis.clone()).await,
        0,
        "the visit's lookup is gone"
    );
    let records: Vec<String> = redis::cmd("KEYS")
        .arg("purestat:s:*")
        .query_async(&mut redis)
        .await
        .expect("KEYS");
    for key in records {
        let json: Option<String> = redis::cmd("GET")
            .arg(&key)
            .query_async(&mut redis)
            .await
            .expect("GET");
        if let Some(json) = json {
            let state: serde_json::Value = serde_json::from_str(&json).unwrap_or_default();
            assert_ne!(
                state["site_id"].as_u64(),
                Some(ch),
                "a visit of the deleted site is still waiting to be written: {key}"
            );
        }
    }
    assert_eq!(
        settles((0, 0), || rows(ch)).await,
        (0, 0),
        "and its event, buffered or written, is gone"
    );
}
