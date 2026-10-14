//! The campaign taxonomy of FR-102 §3b: one table, checked at both ends.
//!
//! roomler-ai keys its sign-ups by `(source, medium, campaign, content)` and
//! reads purestat's visitors by the same key, so the two must spell it the
//! same way. This module is roomler-ai's `crates/core/src/campaign.rs`,
//! ported table for table and rule for rule; the tests pin the cases its
//! tests pin. A change there is a change here.
//!
//! * [`normalize_source`] / [`normalize_medium`]: trim, lowercase, then the
//!   aliases.
//! * [`source_group`] / [`medium_group`]: the canonical value, or `other`.
//!   An unknown value is **kept verbatim** (lowercased) and only *reported*
//!   under `other`, never dropped: a new channel shows up in the numbers
//!   before it has a name in the table.
//! * [`channel_for_referrer_host`]: the organic channel of a visit, from its
//!   referrer's host, matched by suffix, first match wins.
//!
//! One difference, because purestat counts many sites where roomler-ai is
//! one: the `self` channel is the queried site's own hostnames (its domain,
//! `www.` + the domain and its allowed hostnames, each with its subdomains),
//! checked where roomler-ai checks `roomler.ai`. For the roomler.ai site the
//! two functions agree on every host.
//!
//! The reports apply the same rules in ClickHouse ([`normalize_source_sql`],
//! [`channel_sql`] and the rest), generated from these tables, so a report
//! groups rows exactly as these functions classify a value; the integration
//! suite checks the SQL against the Rust on a real ClickHouse.

use serde::Serialize;

use super::scope::SiteScope;
use super::sql::Sql;

/// `utm_source`: the platform, never the account. `instagram` is retired
/// and kept for history.
pub const SOURCES: [&str; 14] = [
    "youtube",
    "tiktok",
    "x",
    "linkedin",
    "facebook",
    "reddit",
    "hn",
    "devto",
    "hashnode",
    "medium",
    "github",
    "newsletter",
    "blog",
    "instagram",
];

/// `utm_medium`: the format.
pub const MEDIUMS: [&str; 8] = [
    "video",
    "post",
    "carousel",
    "comment",
    "community",
    "bio",
    "article",
    "email",
];

/// The group every value outside the table reports under.
pub const OTHER: &str = "other";

/// The spellings seen in the wild, mapped to the canonical value. Applied
/// AFTER lowercasing, so the left side is lowercase.
pub const SOURCE_ALIASES: &[(&str, &str)] = &[
    ("twitter", "x"),
    ("fb", "facebook"),
    ("yt", "youtube"),
    ("youtub", "youtube"),
    ("li", "linkedin"),
    ("ig", "instagram"),
];

/// Trim, lowercase, then the aliases. Never empties a non-empty value and
/// never invents one: an unknown source comes back as itself, lowercased.
pub fn normalize_source(raw: &str) -> String {
    let value = raw.trim().to_lowercase();
    match SOURCE_ALIASES.iter().find(|(alias, _)| *alias == value) {
        Some((_, canonical)) => (*canonical).to_string(),
        None => value,
    }
}

/// Trim and lowercase. The mediums have no aliases worth encoding yet; an
/// unknown one is kept as itself.
pub fn normalize_medium(raw: &str) -> String {
    raw.trim().to_lowercase()
}

/// The canonical source a (normalised) value belongs to, or [`OTHER`].
pub fn source_group(normalised: &str) -> &'static str {
    SOURCES
        .iter()
        .copied()
        .find(|s| *s == normalised)
        .unwrap_or(OTHER)
}

/// The canonical medium a (normalised) value belongs to, or [`OTHER`].
pub fn medium_group(normalised: &str) -> &'static str {
    MEDIUMS
        .iter()
        .copied()
        .find(|m| *m == normalised)
        .unwrap_or(OTHER)
}

/// Where a visit came from, by its referrer host (FR-102 §3b).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    Search,
    AiAssistant,
    Social,
    Community,
    Code,
    /// The site itself: navigation, never a source.
    #[serde(rename = "self")]
    SelfSite,
    /// No referrer at all.
    Direct,
    Other,
}

impl Channel {
    /// The wire / report spelling (the serde one).
    pub fn as_str(self) -> &'static str {
        match self {
            Channel::Search => "search",
            Channel::AiAssistant => "ai_assistant",
            Channel::Social => "social",
            Channel::Community => "community",
            Channel::Code => "code",
            Channel::SelfSite => "self",
            Channel::Direct => "direct",
            Channel::Other => "other",
        }
    }

    /// Every channel, in report order.
    pub const ALL: [Channel; 8] = [
        Channel::Search,
        Channel::AiAssistant,
        Channel::Social,
        Channel::Community,
        Channel::Code,
        Channel::SelfSite,
        Channel::Direct,
        Channel::Other,
    ];
}

/// Hosts matched by suffix (`host == domain` or `host` ends with
/// `.domain`), first match wins, so `gemini.google.com` sits ABOVE the
/// `google` label rule below, and `l.facebook.com` needs no row of its own.
/// roomler-ai's table ends with `roomler.ai` as `self`; here the site's own
/// hostnames take that place (see [`channel_for_referrer_host`]).
pub const HOST_SUFFIXES: &[(&str, Channel)] = &[
    // AI assistants
    ("chatgpt.com", Channel::AiAssistant),
    ("chat.openai.com", Channel::AiAssistant),
    ("perplexity.ai", Channel::AiAssistant),
    ("gemini.google.com", Channel::AiAssistant),
    ("copilot.microsoft.com", Channel::AiAssistant),
    ("claude.ai", Channel::AiAssistant),
    // Search (the `google.*` / `yandex.*` families are a label rule below)
    ("bing.com", Channel::Search),
    ("duckduckgo.com", Channel::Search),
    ("ecosia.org", Channel::Search),
    // Social
    ("t.co", Channel::Social),
    ("x.com", Channel::Social),
    ("twitter.com", Channel::Social),
    ("linkedin.com", Channel::Social),
    ("lnkd.in", Channel::Social),
    ("facebook.com", Channel::Social),
    ("youtube.com", Channel::Social),
    ("youtu.be", Channel::Social),
    ("tiktok.com", Channel::Social),
    ("reddit.com", Channel::Social),
    ("instagram.com", Channel::Social),
    // Community
    ("news.ycombinator.com", Channel::Community),
    ("lobste.rs", Channel::Community),
    ("dev.to", Channel::Community),
    ("hashnode.com", Channel::Community),
    ("medium.com", Channel::Community),
    // Code
    ("github.com", Channel::Code),
];

/// Search engines whose registrable label is followed by any country TLD
/// (`google.com`, `www.google.co.uk`, `yandex.ru`).
pub const SEARCH_LABELS: &[&str] = &["google", "yandex"];

fn is_or_under(host: &str, domain: &str) -> bool {
    host == domain || host.ends_with(&format!(".{domain}"))
}

/// The organic channel of a referrer host. `None` (no referrer recorded) is
/// [`Channel::Direct`]; a host nothing here recognises is
/// [`Channel::Other`]. `own_hosts` are the site's own hostnames, as
/// [`SiteScope::hostnames`] lists them: a referrer on one of them, or under
/// one, is [`Channel::SelfSite`]. A raw `Host`-style value with upper case,
/// a port or a trailing dot is tolerated.
pub fn channel_for_referrer_host(host: Option<&str>, own_hosts: &[String]) -> Channel {
    let Some(raw) = host else {
        return Channel::Direct;
    };
    let host = raw.trim().to_ascii_lowercase();
    let host = host.split(':').next().unwrap_or("").trim_end_matches('.');
    if host.is_empty() {
        return Channel::Direct;
    }
    for (domain, channel) in HOST_SUFFIXES {
        if is_or_under(host, domain) {
            return *channel;
        }
    }
    if own_hosts
        .iter()
        .filter(|own| !own.is_empty())
        .any(|own| is_or_under(host, own))
    {
        return Channel::SelfSite;
    }
    // `google.*`: a `google` label that is not the last one (so `google.com`,
    // `www.google.de`, `google.co.uk`), never a host that merely contains
    // the word (`googleusercontent.com`, `notgoogle.com`).
    let labels: Vec<&str> = host.split('.').collect();
    if labels.len() >= 2
        && labels[..labels.len() - 1]
            .iter()
            .any(|l| SEARCH_LABELS.contains(l))
    {
        return Channel::Search;
    }
    Channel::Other
}

// --- The same rules in ClickHouse ------------------------------------------

/// A string literal for one of the constants above. They are compile-time
/// values of `[a-z0-9._-]`, which is what makes splicing them safe; the
/// assertion keeps it that way. Everything else is bound.
fn lit(value: &str) -> String {
    assert!(
        value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-".contains(&b)),
        "not a table constant: {value:?}"
    );
    format!("'{value}'")
}

fn array_lit<'a>(values: impl IntoIterator<Item = &'a str>) -> String {
    let items: Vec<String> = values.into_iter().map(lit).collect();
    format!("[{}]", items.join(", "))
}

/// `expr` without the whitespace `str::trim` removes at either end: Unicode
/// White_Space, which is ASCII `\t \n \v \f \r`, U+0085 and the separators
/// (`\pZ`). ClickHouse's own `trim` removes spaces only.
pub fn trim_sql(expr: &str) -> String {
    const WS: &str = r"[\\t\\n\\x0B\\f\\r\\x{85}\\pZ]";
    format!("replaceRegexpAll({expr}, '^{WS}+|{WS}+$', '')")
}

/// `normalize_medium` in SQL: trimmed, then lowercased.
pub fn normalize_medium_sql(expr: &str) -> String {
    format!("lowerUTF8({})", trim_sql(expr))
}

/// `normalize_source` in SQL: trimmed, lowercased, then the aliases.
pub fn normalize_source_sql(expr: &str) -> String {
    let lowered = normalize_medium_sql(expr);
    format!(
        "transform({lowered}, {}, {}, {lowered})",
        array_lit(SOURCE_ALIASES.iter().map(|(alias, _)| *alias)),
        array_lit(SOURCE_ALIASES.iter().map(|(_, canonical)| *canonical)),
    )
}

/// `source_group(normalize_source(expr))` in SQL.
pub fn source_group_sql(expr: &str) -> String {
    let sources = array_lit(SOURCES);
    format!(
        "transform({}, {sources}, {sources}, {})",
        normalize_source_sql(expr),
        lit(OTHER)
    )
}

/// `medium_group(normalize_medium(expr))` in SQL.
pub fn medium_group_sql(expr: &str) -> String {
    let mediums = array_lit(MEDIUMS);
    format!(
        "transform({}, {mediums}, {mediums}, {})",
        normalize_medium_sql(expr),
        lit(OTHER)
    )
}

/// The host part of a stored referrer, as [`channel_for_referrer_host`]
/// reads one: the address's authority without credentials, port included
/// (the classifier drops it). A referrer that is not an address with a host
/// is passed whole, so it is a referrer the table does not know (`other`),
/// never a missing one (`direct`).
///
/// A regular expression, because ClickHouse's `domain()` drops a host
/// without a dot (`localhost`) or with a trailing one, and `netloc()` reads
/// `https://a.example/b/c` as `a.example/b`. `\x3F` is `?`, which the SQL
/// text cannot hold: the client takes it for a placeholder.
pub fn referrer_host_sql(referrer: &str) -> String {
    const AUTHORITY: &str = r"'^[A-Za-z][A-Za-z0-9+.-]*://([^@/#\\x3F]*@)*([^/#\\x3F]*)'";
    let host = format!("arrayElement(extractGroups({referrer}, {AUTHORITY}), 2)");
    format!("if(empty({host}), {referrer}, {host})")
}

/// `h = 'domain' OR endsWith(h, '.domain')`, for a table constant.
fn suffix_sql(domain: &str) -> String {
    format!(
        "h = {} OR endsWith(h, {})",
        lit(domain),
        lit(&format!(".{domain}"))
    )
}

/// [`channel_for_referrer_host`] in SQL, for the referrer host `host` (an
/// expression; see [`referrer_host_sql`]) of a row of one of `scopes`. Each
/// site's own hostnames are bound, and a row is `self` only for its own
/// site's (`site_id`).
pub fn channel_sql(host: &str, scopes: &[SiteScope]) -> Sql {
    let mut sql = Sql::new("arrayElement(arrayMap(h -> multiIf(h = '', ");
    sql.push(&lit(Channel::Direct.as_str()));
    for (domain, channel) in HOST_SUFFIXES {
        sql.push(", ")
            .push(&suffix_sql(domain))
            .push(", ")
            .push(&lit(channel.as_str()));
    }
    sql.push(", (0");
    for scope in scopes {
        let own: Vec<&String> = scope.hostnames.iter().filter(|h| !h.is_empty()).collect();
        if own.is_empty() {
            continue;
        }
        sql.push(&format!(" OR (site_id = {} AND (0", scope.ch_id));
        for host in own {
            sql.push(" OR h = ").bind(host.clone());
            sql.push(" OR endsWith(h, ").bind(format!(".{host}"));
            sql.push(")");
        }
        sql.push("))");
    }
    sql.push("), ").push(&lit(Channel::SelfSite.as_str()));
    sql.push(&format!(
        ", arrayExists(l -> has({}, l), arrayPopBack(splitByChar('.', h))), {}, {}), [",
        array_lit(SEARCH_LABELS.iter().copied()),
        lit(Channel::Search.as_str()),
        lit(Channel::Other.as_str()),
    ));
    // The host as the classifier reads it: trimmed, ASCII-lowercased, cut at
    // the first `:`, without trailing dots.
    sql.push(&format!(
        "trim(TRAILING '.' FROM arrayElement(splitByChar(':', lower({})), 1))]), 1)",
        trim_sql(host)
    ));
    sql
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roomler() -> Vec<String> {
        vec!["roomler.ai".to_string()]
    }

    /// AC5's two halves: the aliases and the case, and an unknown value kept
    /// as itself rather than dropped or guessed at.
    #[test]
    fn sources_normalise_through_case_and_aliases() {
        assert_eq!(normalize_source("YouTub"), "youtube");
        assert_eq!(normalize_source(" yt "), "youtube");
        assert_eq!(normalize_source("YouTube"), "youtube");
        assert_eq!(normalize_source("Twitter"), "x");
        assert_eq!(normalize_source("FB"), "facebook");
        assert_eq!(normalize_source("li"), "linkedin");
        assert_eq!(normalize_source("IG"), "instagram");
        // Unknown: verbatim, lowercased — never dropped.
        assert_eq!(normalize_source("Mastodon"), "mastodon");
        assert_eq!(normalize_source("check"), "check");
        // A canonical value is a fixed point.
        for s in SOURCES {
            assert_eq!(normalize_source(s), s);
            assert_eq!(normalize_source(&s.to_uppercase()), s);
        }
    }

    #[test]
    fn mediums_normalise_through_case() {
        assert_eq!(normalize_medium("Video"), "video");
        assert_eq!(normalize_medium(" POST "), "post");
        assert_eq!(normalize_medium("reel"), "reel");
        for m in MEDIUMS {
            assert_eq!(normalize_medium(m), m);
            assert_eq!(normalize_medium(&m.to_uppercase()), m);
        }
    }

    /// The whole table, both directions: every canonical value groups to
    /// itself, every alias groups to its canonical, and anything else is
    /// `other`.
    #[test]
    fn the_table_is_exhaustive_and_unknowns_group_under_other() {
        for s in SOURCES {
            assert_eq!(source_group(s), s);
        }
        for (alias, canonical) in SOURCE_ALIASES {
            assert!(
                SOURCES.contains(canonical),
                "alias {alias} points outside the table: {canonical}"
            );
            assert_eq!(source_group(&normalize_source(alias)), *canonical);
        }
        for m in MEDIUMS {
            assert_eq!(medium_group(m), m);
        }
        assert_eq!(source_group("mastodon"), OTHER);
        assert_eq!(source_group(""), OTHER);
        assert_eq!(medium_group("reel"), OTHER);
        // The table has no duplicates (a duplicate would make `source_group`
        // depend on order).
        let mut sorted: Vec<&str> = SOURCES.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), SOURCES.len());
        let mut sorted: Vec<&str> = MEDIUMS.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), MEDIUMS.len());
        // An alias must not shadow a canonical value.
        for (alias, _) in SOURCE_ALIASES {
            assert!(
                !SOURCES.contains(alias),
                "{alias} is both an alias and canonical"
            );
        }
    }

    /// The spec's §3b table, host by host, plus the subdomain and port forms
    /// the referrer actually arrives in: roomler-ai's cases, with roomler.ai
    /// as the site.
    #[test]
    fn referrer_hosts_map_to_their_channel() {
        use Channel::*;
        let cases: &[(&str, Channel)] = &[
            // search
            ("google.com", Search),
            ("www.google.com", Search),
            ("www.google.co.uk", Search),
            ("google.de", Search),
            ("bing.com", Search),
            ("www.bing.com", Search),
            ("duckduckgo.com", Search),
            ("ecosia.org", Search),
            ("yandex.ru", Search),
            ("www.yandex.com", Search),
            // AI assistants
            ("chatgpt.com", AiAssistant),
            ("chat.openai.com", AiAssistant),
            ("perplexity.ai", AiAssistant),
            ("www.perplexity.ai", AiAssistant),
            ("gemini.google.com", AiAssistant),
            ("copilot.microsoft.com", AiAssistant),
            ("claude.ai", AiAssistant),
            // social
            ("t.co", Social),
            ("x.com", Social),
            ("twitter.com", Social),
            ("linkedin.com", Social),
            ("www.linkedin.com", Social),
            ("lnkd.in", Social),
            ("facebook.com", Social),
            ("l.facebook.com", Social),
            ("m.facebook.com", Social),
            ("youtube.com", Social),
            ("www.youtube.com", Social),
            ("m.youtube.com", Social),
            ("youtu.be", Social),
            ("tiktok.com", Social),
            ("www.tiktok.com", Social),
            ("reddit.com", Social),
            ("old.reddit.com", Social),
            ("out.reddit.com", Social),
            ("instagram.com", Social),
            // community
            ("news.ycombinator.com", Community),
            ("lobste.rs", Community),
            ("dev.to", Community),
            ("hashnode.com", Community),
            ("medium.com", Community),
            // code
            ("github.com", Code),
            // self
            ("roomler.ai", SelfSite),
            ("www.roomler.ai", SelfSite),
            // other
            ("example.com", Other),
            ("localhost", Other),
            ("localhost:5000", Other),
            ("googleusercontent.com", Other),
            ("notgoogle.com", Other),
            ("google", Other),
            ("roomler.ai.evil.example", Other),
        ];
        for (host, expected) in cases {
            assert_eq!(
                channel_for_referrer_host(Some(host), &roomler()),
                *expected,
                "{host}"
            );
        }
        // The forms a raw referrer can arrive in.
        assert_eq!(
            channel_for_referrer_host(Some("WWW.YouTube.com:443"), &roomler()),
            Social
        );
        assert_eq!(
            channel_for_referrer_host(Some("github.com."), &roomler()),
            Code
        );
        // No referrer is direct, not other.
        assert_eq!(channel_for_referrer_host(None, &roomler()), Direct);
        assert_eq!(channel_for_referrer_host(Some(""), &roomler()), Direct);
        assert_eq!(channel_for_referrer_host(Some("   "), &roomler()), Direct);
    }

    #[test]
    fn channel_spellings_are_the_wire_names() {
        for c in Channel::ALL {
            let json = serde_json::to_string(&c).unwrap();
            assert_eq!(json, format!("\"{}\"", c.as_str()));
        }
        assert_eq!(Channel::SelfSite.as_str(), "self");
        assert_eq!(Channel::AiAssistant.as_str(), "ai_assistant");
    }

    /// `self` is whichever site is queried: its domain, `www.` + the domain
    /// and its allowed hostnames, each with its subdomains. Another site's
    /// domain is just another referrer, and a table row still wins over the
    /// site's own hostnames, as `roomler.ai` sits last in roomler-ai's table.
    #[test]
    fn self_is_the_queried_sites_own_hostnames() {
        let own = vec![
            "example.com".to_string(),
            "www.example.com".to_string(),
            "staging.example.net".to_string(),
        ];
        for host in [
            "example.com",
            "www.example.com",
            "blog.example.com",
            "staging.example.net",
            "a.staging.example.net",
        ] {
            assert_eq!(
                channel_for_referrer_host(Some(host), &own),
                Channel::SelfSite,
                "{host}"
            );
        }
        for host in ["example.net", "roomler.ai", "example.com.evil.example"] {
            assert_eq!(
                channel_for_referrer_host(Some(host), &own),
                Channel::Other,
                "{host}"
            );
        }
        assert_eq!(
            channel_for_referrer_host(Some("www.roomler.ai"), &[]),
            Channel::Other,
            "no site, no self"
        );
        // A site on a host the table knows is still that channel.
        let on_github = vec!["github.com".to_string()];
        assert_eq!(
            channel_for_referrer_host(Some("github.com"), &on_github),
            Channel::Code
        );
        assert_eq!(
            channel_for_referrer_host(Some(""), &own),
            Channel::Direct,
            "an empty own host matches nothing"
        );
    }

    #[test]
    fn every_table_constant_is_safe_to_splice() {
        for value in SOURCES
            .iter()
            .chain(MEDIUMS.iter())
            .chain(SEARCH_LABELS.iter())
            .chain(SOURCE_ALIASES.iter().flat_map(|(a, c)| [a, c]))
            .chain(HOST_SUFFIXES.iter().map(|(d, _)| d))
            .copied()
            .chain(Channel::ALL.iter().map(|c| c.as_str()))
        {
            lit(value);
        }
    }

    #[test]
    fn the_sql_splices_only_table_constants_and_binds_each_site_host() {
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
        let sql = channel_sql(&referrer_host_sql("referrer"), &scopes);
        assert_eq!(sql.text.matches('?').count(), sql.binds.len());
        assert_eq!(
            sql.binds,
            [
                "a.example",
                ".a.example",
                "www.a.example",
                ".www.a.example",
                "b'; DROP TABLE events; --",
                ".b'; DROP TABLE events; --"
            ]
        );
        assert!(!sql.text.contains("DROP"), "{}", sql.text);
        assert!(
            sql.text.contains("(site_id = 7 AND (0 OR h = ?"),
            "{}",
            sql.text
        );
        assert!(
            sql.text.contains("(site_id = 9 AND (0 OR h = ?"),
            "{}",
            sql.text
        );
        // Every table row is in the expression, in order, before `self`.
        let mut at = 0;
        for (domain, _) in HOST_SUFFIXES {
            let row = format!("h = '{domain}' OR endsWith(h, '.{domain}')");
            let found = sql.text[at..]
                .find(&row)
                .unwrap_or_else(|| panic!("{domain} missing or out of order"));
            at += found + row.len();
        }
        assert!(sql.text[at..].contains("'self'"));
        assert!(sql.text.contains("has(['google', 'yandex'], l)"));
    }

    #[test]
    fn a_site_without_hostnames_has_no_self() {
        let none = SiteScope {
            ch_id: 7,
            hostnames: vec![],
        };
        let sql = channel_sql("host", &[none]);
        assert!(sql.binds.is_empty());
        assert!(sql.text.contains(", (0), 'self'"), "{}", sql.text);
    }

    #[test]
    fn the_group_sql_lists_the_tables() {
        let sql = source_group_sql("utm_source");
        assert!(
            sql.contains("['twitter', 'fb', 'yt', 'youtub', 'li', 'ig'], ['x', 'facebook', 'youtube', 'youtube', 'linkedin', 'instagram']"),
            "{sql}"
        );
        assert!(sql.ends_with(", 'other')"), "{sql}");
        assert!(medium_group_sql("utm_medium").contains(
            "['video', 'post', 'carousel', 'comment', 'community', 'bio', 'article', 'email']"
        ));
        assert!(!sql.contains('?'), "nothing to bind: {sql}");
    }
}
