//! What counts for a site: the hostnames its pages may carry, and the site as
//! a ClickHouse query sees it.
//!
//! A site's events are the ones whose page hostname is the site's domain,
//! `www.` + the domain, or one of its `allowed_hostnames`. Ingest drops the
//! rest, and every read is scoped by the same list, so history is read
//! through the rule that new rows are written by.

use purestat_db::models::Site;
use std::net::IpAddr;

use super::sql::Sql;

/// A hostname as the allowlist compares it: without a port or a trailing
/// dot, and parsed the way a browser parses a URL's host, which is also how
/// ingest reads a page's host: lowercase, an internationalized name in its
/// `xn--` form, an IPv4 address in dotted form, an IPv6 literal in brackets.
/// Text that is not a host comes back trimmed and lowercased.
pub fn normalize_hostname(host: &str) -> String {
    let host = host.trim();
    let host = if host.starts_with('[') {
        match host.find(']') {
            Some(end) => &host[..=end],
            None => host,
        }
    } else {
        match host.rsplit_once(':') {
            Some((name, port))
                if !name.contains(':')
                    && !port.is_empty()
                    && port.bytes().all(|b| b.is_ascii_digit()) =>
            {
                name
            }
            _ => host,
        }
    };
    let host = host.trim_end_matches('.');
    match url::Host::parse(host) {
        Ok(parsed) => parsed.to_string(),
        Err(_) => host.to_lowercase(),
    }
}

/// A host that is never a visitor's: an empty one, `localhost`, a `.local`
/// name, or an IP literal that is loopback, private, link-local,
/// carrier-grade NAT or unspecified. Pages on these are dev servers and test
/// runs, and no site may allow them.
pub fn is_local_or_private(host: &str) -> bool {
    let host = normalize_hostname(host);
    if host.is_empty()
        || host == "localhost"
        || host.ends_with(".localhost")
        || host == "local"
        || host.ends_with(".local")
    {
        return true;
    }
    let literal = host
        .strip_prefix('[')
        .and_then(|h| h.strip_suffix(']'))
        .unwrap_or(&host);
    literal.parse::<IpAddr>().is_ok_and(|ip| is_private_ip(&ip))
}

/// Loopback, private (RFC 1918), link-local, carrier-grade NAT
/// (`100.64.0.0/10`), unique-local, or unspecified.
pub fn is_private_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let [a, b, ..] = v4.octets();
            v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_unspecified()
                || (a == 100 && (b & 0xC0) == 64)
        }
        IpAddr::V6(v6) => {
            let first = v6.segments()[0];
            v6.is_loopback()
                || v6.is_unspecified()
                || (first & 0xfe00) == 0xfc00
                || (first & 0xffc0) == 0xfe80
                || v6
                    .to_ipv4_mapped()
                    .is_some_and(|v4| is_private_ip(&IpAddr::V4(v4)))
        }
    }
}

/// The hostnames whose pages count for a site: its domain, `www.` + the
/// domain, and its `allowed_hostnames`, normalized and without duplicates.
/// A local or private host is never on the list, whatever the site says.
pub fn counted_hostnames(site: &Site) -> Vec<String> {
    let domain = normalize_hostname(&site.domain);
    let mut hosts = Vec::new();
    if !domain.is_empty() {
        hosts.push(normalize_hostname(&format!("www.{domain}")));
        hosts.insert(0, domain);
    }
    hosts.extend(site.allowed_hostnames.iter().map(|h| normalize_hostname(h)));
    let mut seen = std::collections::HashSet::new();
    hosts.retain(|h| !is_local_or_private(h) && seen.insert(h.clone()));
    hosts
}

/// A site as a ClickHouse query sees it: the `site_id` its rows carry, and
/// the hostnames among those rows that count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiteScope {
    pub ch_id: u64,
    pub hostnames: Vec<String>,
}

impl SiteScope {
    pub fn for_site(site: &Site) -> Self {
        Self {
            ch_id: site.clickhouse_id(),
            hostnames: counted_hostnames(site),
        }
    }

    /// Whether a page on `host` counts for the site.
    pub fn counts(&self, host: &str) -> bool {
        let host = normalize_hostname(host);
        !host.is_empty() && self.hostnames.contains(&host)
    }
}

/// The predicate selecting the rows of these sites in `events`:
/// `((site_id = 1 AND hostname IN (?, ?)) OR (...))`. The ids are u64s the
/// route resolved within the caller's org, never caller text, so they are
/// literals; the hostnames are bound. A site with no hostname that counts,
/// like an empty list of sites, selects nothing.
pub fn events_scope(scopes: &[SiteScope]) -> Sql {
    let mut sql = Sql::new("(");
    if scopes.is_empty() {
        sql.push("0");
    }
    for (i, scope) in scopes.iter().enumerate() {
        if i > 0 {
            sql.push(" OR ");
        }
        if scope.hostnames.is_empty() {
            sql.push(&format!("(site_id = {} AND 0)", scope.ch_id));
            continue;
        }
        sql.push(&format!("(site_id = {} AND hostname IN ", scope.ch_id));
        sql.bind_list(&scope.hostnames);
        sql.push(")");
    }
    sql.push(")");
    sql
}

/// `1, 2, 3`: the ids as a list for `site_id IN (...)` on `sessions`, which
/// has no hostname column (see `sql::sessions_where`). An empty list of
/// sites gives `0`, which no site has.
pub fn site_ids(scopes: &[SiteScope]) -> String {
    if scopes.is_empty() {
        return "0".to_string();
    }
    scopes
        .iter()
        .map(|s| s.ch_id.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use bson::{DateTime, oid::ObjectId};

    fn site(domain: &str, allowed: &[&str]) -> Site {
        Site {
            id: Some(ObjectId::new()),
            org_id: ObjectId::new(),
            domain: domain.into(),
            name: "x".into(),
            timezone: "UTC".into(),
            is_public: false,
            allowed_hostnames: allowed.iter().map(|s| s.to_string()).collect(),
            ch_id: Some(7),
            created_at: DateTime::now(),
            updated_at: DateTime::now(),
        }
    }

    #[test]
    fn hostnames_are_normalized() {
        assert_eq!(normalize_hostname(" Example.COM. "), "example.com");
        assert_eq!(normalize_hostname("example.com:8080"), "example.com");
        assert_eq!(normalize_hostname("example.com.:8080"), "example.com");
        assert_eq!(normalize_hostname("[::1]:8080"), "[::1]");
        assert_eq!(normalize_hostname("[::1]"), "[::1]");
        assert_eq!(normalize_hostname("[0:0::1]"), "[::1]");
        assert_eq!(normalize_hostname("::1"), "::1");
        assert_eq!(normalize_hostname("127.1"), "127.0.0.1");
        assert_eq!(normalize_hostname(""), "");
    }

    #[test]
    fn a_hostname_is_read_as_ingest_reads_a_page_address() {
        // Ingest takes the host from `url::Url::parse(page).host_str()`; a
        // site's own spelling of the same host must compare equal to it.
        for (written, page) in [
            ("Bücher.de", "https://bücher.de/katalog"),
            ("EXAMPLE.com", "https://Example.COM/"),
            ("0x7f.0.0.1", "http://127.0.0.1/"),
        ] {
            let host = url::Url::parse(page)
                .unwrap()
                .host_str()
                .unwrap()
                .to_string();
            assert_eq!(
                normalize_hostname(written),
                normalize_hostname(&host),
                "{written} vs {page}"
            );
        }
        assert_eq!(normalize_hostname("Bücher.de"), "xn--bcher-kva.de");
    }

    #[test]
    fn local_and_private_hosts_are_never_counted() {
        for host in [
            "",
            "localhost",
            "LOCALHOST",
            "app.localhost",
            "localhost:5173",
            "127.0.0.1",
            "127.0.0.1:3000",
            "0.0.0.0",
            "10.1.2.3",
            "172.16.0.9",
            "172.31.255.1",
            "192.168.1.20",
            "169.254.10.1",
            "100.64.0.7",
            "100.127.1.1",
            "[::1]",
            "::1",
            "[fd00::2]",
            "[fe80::1]",
            "[::ffff:10.0.0.1]",
            "mybox.local",
            "printer.LOCAL",
        ] {
            assert!(is_local_or_private(host), "{host:?} is local or private");
        }
        for host in [
            "roomler.ai",
            "www.roomler.ai",
            "172.32.0.1",
            "100.128.0.1",
            "8.8.8.8",
            "[2001:db8::1]",
            "localhost.example.com",
            "notlocal.io",
            "local.example.com",
        ] {
            assert!(!is_local_or_private(host), "{host:?} is a real host");
        }
    }

    #[test]
    fn a_site_counts_its_domain_www_and_allowed_hostnames_once_each() {
        let s = site(
            "Roomler.ai",
            &[
                "staging.roomler.ai",
                "WWW.roomler.ai",
                " ",
                "docs.roomler.ai:443",
            ],
        );
        assert_eq!(
            counted_hostnames(&s),
            [
                "roomler.ai",
                "www.roomler.ai",
                "staging.roomler.ai",
                "docs.roomler.ai"
            ]
        );
        let scope = SiteScope::for_site(&s);
        assert_eq!(scope.ch_id, 7);
        assert!(scope.counts("roomler.ai"));
        assert!(scope.counts("WWW.ROOMLER.AI"));
        assert!(scope.counts("staging.roomler.ai"));
        assert!(!scope.counts("blog.roomler.ai"));
        assert!(!scope.counts("evil.example"));
        assert!(!scope.counts(""));
    }

    #[test]
    fn a_site_cannot_allow_a_local_or_private_host() {
        let s = site(
            "example.com",
            &[
                "localhost",
                "127.0.0.1",
                "192.168.1.20",
                "dev.local",
                "staging.example.com",
            ],
        );
        assert_eq!(
            counted_hostnames(&s),
            ["example.com", "www.example.com", "staging.example.com"]
        );
    }

    #[test]
    fn an_empty_scope_selects_nothing() {
        let none = SiteScope {
            ch_id: 7,
            hostnames: vec![],
        };
        let sql = events_scope(&[none]);
        assert_eq!(sql.text, "((site_id = 7 AND 0))");
        assert!(sql.binds.is_empty());
        assert_eq!(events_scope(&[]).text, "(0)");
        assert_eq!(site_ids(&[]), "0");
    }

    #[test]
    fn the_events_scope_binds_every_hostname_and_splices_only_the_ids() {
        let scopes = [
            SiteScope {
                ch_id: 7,
                hostnames: vec!["a.example".into(), "www.a.example".into()],
            },
            SiteScope {
                ch_id: 9,
                hostnames: vec!["b'; DROP TABLE events; --".into()],
            },
        ];
        let sql = events_scope(&scopes);
        assert_eq!(
            sql.text,
            "((site_id = 7 AND hostname IN (?, ?)) OR (site_id = 9 AND hostname IN (?)))"
        );
        assert_eq!(
            sql.binds,
            ["a.example", "www.a.example", "b'; DROP TABLE events; --"]
        );
        assert_eq!(site_ids(&scopes), "7, 9");
    }
}
