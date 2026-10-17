//! Deleting a site deletes its rows in ClickHouse (roomler-ai FR-102 PS0b).
//!
//! A site's rows are the ones its reads count (`scope`), and none that another
//! live site's reads count. Two kinds of site exist:
//!
//! - **A site with an id of its own**: every site created since PS0a, and an
//!   older one that shares its id with no live site. Every row carrying its
//!   `site_id` is its own, in both tables, including the rows no read counts
//!   (a dev server's, another host's): its tracker sent them all the same.
//! - **A site that shares its id.** Sites created before PS0a derive the id
//!   from their ObjectId (`Site::legacy_clickhouse_id`), so sites created in
//!   the same second share one, and `DELETE WHERE site_id = …` would take the
//!   other sites' rows too. Only what the site's own hostnames account for
//!   can be told apart: the events on a hostname that is the site's and no
//!   sharer's, and the visits made of such events and none of a sharer's.
//!   The rest stays: a sharer's rows are the sharer's, and rows on a host no
//!   site counts are read by no one.
//!
//! The deletion is one mutation per table (`ALTER TABLE … DELETE`). ClickHouse
//! rewrites the affected parts in the background, so the rows leave the disk
//! rather than being masked. A mutation covers the rows present when it is
//! submitted, so the site route submits the deletion twice: once before the
//! site's Mongo row goes, so that a ClickHouse that refuses it leaves the site
//! as it was, and once after the site's buffered events and live visits are
//! dropped, for the rows written in between.
//!
//! ⚠️ The app's ClickHouse user needs `ALTER DELETE` on every table below
//! (purestat-deploy, the `clickhouse-app-user` users.d file). A table added
//! to the schema needs a statement here and that grant, or a deletion leaves
//! its rows; `every_table_in_the_schema_is_covered` fails until it has one.

use std::collections::BTreeSet;

use clickhouse::Client;
use purestat_db::clickhouse::schemas::Event;
use purestat_db::models::Site;

use super::scope::{counted_hostnames, normalize_hostname};
use super::sql::Sql;

/// The rows of a site being deleted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiteRows {
    /// The `site_id` the rows carry.
    pub site_id: u64,
    /// `None` when no other live site has the id, so every row with it is the
    /// site's. Otherwise the hostnames that are the site's alone: its rows
    /// are the events on them, and the visits made of such events.
    pub hostnames: Option<Vec<String>>,
    /// The hostnames the sharers count: a visit with an event on one of them
    /// is read by a sharer, so it stays.
    pub kept_hostnames: Vec<String>,
}

/// One mutation, and whether to wait for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Statement {
    pub sql: Sql,
    /// Submit with `mutations_sync = 1`, waiting until the rows are gone.
    /// Only the visits of a shared id wait: their statement reads `events`,
    /// so it must finish before the events' own deletion is submitted.
    pub wait: bool,
}

impl SiteRows {
    /// `sharers`: the other live sites whose rows carry the same id
    /// (`SiteDao::sharing_clickhouse_id`).
    pub fn of(site: &Site, sharers: &[Site]) -> Self {
        let site_id = site.clickhouse_id();
        let sharers: Vec<&Site> = sharers
            .iter()
            .filter(|s| s.id != site.id && s.clickhouse_id() == site_id)
            .collect();
        if sharers.is_empty() {
            return Self {
                site_id,
                hostnames: None,
                kept_hostnames: vec![],
            };
        }
        let kept: BTreeSet<String> = sharers.iter().flat_map(|s| counted_hostnames(s)).collect();
        let own = counted_hostnames(site)
            .into_iter()
            .filter(|h| !kept.contains(h))
            .collect();
        Self {
            site_id,
            hostnames: Some(own),
            kept_hostnames: kept.into_iter().collect(),
        }
    }

    /// Whether no other live site has the id.
    pub fn owns_the_id(&self) -> bool {
        self.hostnames.is_none()
    }

    /// Whether the site has rows that can be told apart from its sharers'.
    pub fn is_empty(&self) -> bool {
        matches!(&self.hostnames, Some(hosts) if hosts.is_empty())
    }

    /// Whether a buffered event is one of the site's rows.
    pub fn owns_event(&self, event: &Event) -> bool {
        if event.site_id != self.site_id {
            return false;
        }
        match &self.hostnames {
            None => true,
            Some(hosts) => hosts.contains(&normalize_hostname(&event.hostname)),
        }
    }

    /// The mutations, in the order they must be submitted.
    pub fn statements(&self) -> Vec<Statement> {
        let id = self.site_id;
        let Some(hosts) = &self.hostnames else {
            return vec![
                Statement {
                    sql: Sql::new(format!("ALTER TABLE events DELETE WHERE site_id = {id}")),
                    wait: false,
                },
                Statement {
                    sql: Sql::new(format!("ALTER TABLE sessions DELETE WHERE site_id = {id}")),
                    wait: false,
                },
            ];
        };
        if hosts.is_empty() {
            return vec![];
        }

        let mut sessions = Sql::new(format!(
            "ALTER TABLE sessions DELETE WHERE site_id = {id} AND session_id IN \
             (SELECT session_id FROM events WHERE site_id = {id} AND hostname IN "
        ));
        sessions.bind_list(hosts);
        sessions.push(")");
        if !self.kept_hostnames.is_empty() {
            sessions.push(&format!(
                " AND session_id NOT IN (SELECT session_id FROM events WHERE site_id = {id} AND hostname IN "
            ));
            sessions.bind_list(&self.kept_hostnames);
            sessions.push(")");
        }

        let mut events = Sql::new(format!(
            "ALTER TABLE events DELETE WHERE site_id = {id} AND hostname IN "
        ));
        events.bind_list(hosts);

        vec![
            Statement {
                sql: sessions,
                wait: true,
            },
            Statement {
                sql: events,
                wait: false,
            },
        ]
    }
}

/// Submits the deletions.
pub struct PurgeService {
    client: Client,
}

impl PurgeService {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    /// Submits every statement in order. An `ALTER … DELETE` returns once the
    /// mutation is recorded (`mutations_sync = 0`); its progress is in
    /// `system.mutations`. The first refusal is returned and the rest are
    /// not submitted.
    pub async fn submit(&self, rows: &SiteRows) -> Result<(), clickhouse::error::Error> {
        for statement in rows.statements() {
            statement
                .sql
                .query(&self.client)
                .with_option("mutations_sync", if statement.wait { "1" } else { "0" })
                .execute()
                .await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bson::{DateTime, oid::ObjectId};

    fn site(id: [u8; 12], ch_id: Option<u64>, domain: &str, allowed: &[&str]) -> Site {
        Site {
            id: Some(ObjectId::from_bytes(id)),
            org_id: ObjectId::new(),
            domain: domain.into(),
            name: "x".into(),
            timezone: "UTC".into(),
            is_public: false,
            allowed_hostnames: allowed.iter().map(|s| s.to_string()).collect(),
            ch_id,
            created_at: DateTime::now(),
            updated_at: DateTime::now(),
        }
    }

    /// Two ObjectIds from one second and one process: their first 8 bytes,
    /// and so their legacy ClickHouse id, are the same.
    const A: [u8; 12] = [
        0x65, 0x00, 0x00, 0x01, 0x0a, 0x0b, 0x0c, 0x0d, 0x00, 0x00, 0x00, 0x01,
    ];
    const B: [u8; 12] = [
        0x65, 0x00, 0x00, 0x01, 0x0a, 0x0b, 0x0c, 0x0d, 0x00, 0x00, 0x00, 0x02,
    ];

    fn event(site_id: u64, host: &str) -> Event {
        Event {
            site_id,
            visitor_hash: "v".into(),
            session_id: "s".into(),
            event_name: "pageview".into(),
            url: format!("https://{host}/"),
            path: "/".into(),
            hostname: host.into(),
            referrer: String::new(),
            referrer_source: String::new(),
            utm_source: String::new(),
            utm_medium: String::new(),
            utm_campaign: String::new(),
            utm_content: String::new(),
            utm_term: String::new(),
            country: String::new(),
            region: String::new(),
            city: String::new(),
            browser: String::new(),
            browser_version: String::new(),
            os: String::new(),
            os_version: String::new(),
            device_type: String::new(),
            screen_width: 0,
            screen_height: 0,
            prop_keys: vec![],
            prop_values: vec![],
            revenue_amount: None,
            revenue_currency: None,
            timestamp: time::OffsetDateTime::now_utc(),
        }
    }

    #[test]
    fn a_site_with_an_id_of_its_own_loses_every_row_with_it() {
        let s = site(A, Some(42), "a.example", &[]);
        let rows = SiteRows::of(&s, &[]);
        assert!(rows.owns_the_id());
        let statements = rows.statements();
        let texts: Vec<&str> = statements.iter().map(|s| s.sql.text.as_str()).collect();
        assert_eq!(
            texts,
            [
                "ALTER TABLE events DELETE WHERE site_id = 42",
                "ALTER TABLE sessions DELETE WHERE site_id = 42",
            ]
        );
        assert!(statements.iter().all(|s| !s.wait && s.sql.binds.is_empty()));
        assert!(
            rows.owns_event(&event(42, "localhost")),
            "a row no read counts is still its tracker's"
        );
        assert!(!rows.owns_event(&event(43, "a.example")));
    }

    #[test]
    fn a_legacy_site_that_shares_no_id_loses_every_row_with_it() {
        let s = site(A, None, "a.example", &[]);
        let other = site([7; 12], None, "b.example", &[]);
        let rows = SiteRows::of(&s, &[other]);
        assert!(rows.owns_the_id(), "a site with another id is not a sharer");
        assert_eq!(
            rows.site_id,
            Site::legacy_clickhouse_id(&ObjectId::from_bytes(A))
        );
    }

    #[test]
    fn a_shared_id_deletes_only_what_the_sites_own_hostnames_account_for() {
        let doomed = site(
            A,
            None,
            "a.example",
            &["Staging.Shared.example", "b.example"],
        );
        let sharer = site(B, None, "b.example", &["staging.shared.example"]);
        let rows = SiteRows::of(&doomed, &[sharer]);
        let id = Site::legacy_clickhouse_id(&ObjectId::from_bytes(A));
        assert_eq!(rows.site_id, id);
        assert_eq!(
            rows.hostnames,
            Some(vec!["a.example".to_string(), "www.a.example".to_string()]),
            "a hostname a sharer counts stays the sharer's"
        );
        assert_eq!(
            rows.kept_hostnames,
            ["b.example", "staging.shared.example", "www.b.example"]
        );

        let statements = rows.statements();
        assert_eq!(statements.len(), 2);
        assert!(
            statements[0].wait,
            "the visits go first, and are waited for: they read events"
        );
        assert_eq!(
            statements[0].sql.text,
            format!(
                "ALTER TABLE sessions DELETE WHERE site_id = {id} AND session_id IN \
                 (SELECT session_id FROM events WHERE site_id = {id} AND hostname IN (?, ?)) \
                 AND session_id NOT IN (SELECT session_id FROM events WHERE site_id = {id} AND hostname IN (?, ?, ?))"
            )
        );
        assert_eq!(
            statements[0].sql.binds,
            [
                "a.example",
                "www.a.example",
                "b.example",
                "staging.shared.example",
                "www.b.example"
            ]
        );
        assert!(!statements[1].wait);
        assert_eq!(
            statements[1].sql.text,
            format!("ALTER TABLE events DELETE WHERE site_id = {id} AND hostname IN (?, ?)")
        );
        assert_eq!(statements[1].sql.binds, ["a.example", "www.a.example"]);

        assert!(rows.owns_event(&event(id, "WWW.A.example")));
        assert!(!rows.owns_event(&event(id, "b.example")), "the sharer's");
        assert!(
            !rows.owns_event(&event(id, "localhost")),
            "no one's to delete"
        );
    }

    #[test]
    fn a_shared_id_with_no_hostname_of_its_own_deletes_nothing() {
        let doomed = site(A, None, "b.example", &[]);
        let sharer = site(B, None, "b.example", &[]);
        let rows = SiteRows::of(&doomed, &[sharer]);
        assert!(rows.is_empty());
        assert!(rows.statements().is_empty());
        assert!(!rows.owns_event(&event(rows.site_id, "b.example")));
    }

    #[test]
    fn a_hostname_is_bound_never_spliced() {
        let doomed = site(A, None, "a.example", &["x') OR 1=1 --"]);
        let sharer = site(B, None, "b.example", &[]);
        for statement in SiteRows::of(&doomed, &[sharer]).statements() {
            assert!(
                !statement.sql.text.contains("OR 1=1"),
                "{}",
                statement.sql.text
            );
            assert_eq!(
                statement.sql.text.matches('?').count(),
                statement.sql.binds.len()
            );
        }
    }

    /// Every table the schema creates is deleted from: a new table without a
    /// statement here would keep a deleted site's rows.
    #[test]
    fn every_table_in_the_schema_is_covered() {
        let schema = include_str!("../../../../scripts/clickhouse-init.sql");
        let tables: Vec<&str> = schema
            .split("CREATE TABLE IF NOT EXISTS ")
            .skip(1)
            .map(|rest| {
                rest.split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '.'))
                    .next()
                    .unwrap_or("")
            })
            .map(|name| name.trim_start_matches("purestat."))
            .collect();
        assert!(!tables.is_empty(), "the schema names its tables");
        let rows = SiteRows::of(&site(A, Some(42), "a.example", &[]), &[]);
        let statements: Vec<String> = rows.statements().into_iter().map(|s| s.sql.text).collect();
        for table in tables {
            let prefix = format!("ALTER TABLE {table} DELETE WHERE site_id = 42");
            assert!(
                statements.iter().any(|s| s == &prefix),
                "no deletion for table {table}: {statements:?}"
            );
        }
    }
}
