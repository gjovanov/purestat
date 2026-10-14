//! The SQL vocabulary every analytics query shares: a buffer that keeps bind
//! values beside the `?` they fill, the dimensions and their columns on each
//! table, filters, and the WHERE clauses over `events` and `sessions`.
//!
//! Nothing a request sends reaches the SQL text. Columns come from the
//! `Dimension` allowlist, dates are parsed strictly before they get here, and
//! every value travels as a bind parameter (GHSA-7f5h-5qwr-rxh5).

use std::sync::LazyLock;

use clickhouse::Client;

use super::campaign;
use super::scope::{SiteScope, events_scope, site_ids};

/// The campaign columns as the reports read them: normalised and grouped by
/// the campaign taxonomy (`campaign`). `events` and `sessions` name the
/// columns alike.
static UTM_SOURCE: LazyLock<String> =
    LazyLock::new(|| campaign::normalize_source_sql("utm_source"));
static UTM_MEDIUM: LazyLock<String> =
    LazyLock::new(|| campaign::normalize_medium_sql("utm_medium"));
static UTM_SOURCE_GROUP: LazyLock<String> =
    LazyLock::new(|| campaign::source_group_sql("utm_source"));
static UTM_MEDIUM_GROUP: LazyLock<String> =
    LazyLock::new(|| campaign::medium_group_sql("utm_medium"));
static REFERRER_HOST: LazyLock<String> = LazyLock::new(|| campaign::referrer_host_sql("referrer"));

/// The two tables a dimension can be read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Table {
    Events,
    Sessions,
}

/// SQL text and the values of its `?` placeholders, in order. `push` adds
/// text that must not contain a placeholder; `bind` adds one with its value.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sql {
    pub text: String,
    pub binds: Vec<String>,
}

impl Sql {
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        debug_assert!(
            !text.contains('?'),
            "placeholders go through bind(): {text}"
        );
        Self {
            text,
            binds: Vec::new(),
        }
    }

    pub fn push(&mut self, text: &str) -> &mut Self {
        debug_assert!(
            !text.contains('?'),
            "placeholders go through bind(): {text}"
        );
        self.text.push_str(text);
        self
    }

    pub fn bind(&mut self, value: impl Into<String>) -> &mut Self {
        self.text.push('?');
        self.binds.push(value.into());
        self
    }

    /// `(?, ?, ?)` with one bind per value.
    pub fn bind_list(&mut self, values: &[String]) -> &mut Self {
        self.text.push('(');
        for (i, value) in values.iter().enumerate() {
            if i > 0 {
                self.text.push_str(", ");
            }
            self.bind(value.clone());
        }
        self.text.push(')');
        self
    }

    pub fn append(&mut self, other: &Sql) -> &mut Self {
        self.text.push_str(&other.text);
        self.binds.extend(other.binds.iter().cloned());
        self
    }

    /// The query with every value bound, ready to fetch.
    pub fn query(&self, client: &Client) -> clickhouse::query::Query {
        self.binds
            .iter()
            .fold(client.query(&self.text), |q, b| q.bind(b.as_str()))
    }
}

/// A dimension a query may group or filter by, and where it lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dimension {
    Date,
    Hostname,
    Path,
    EntryPage,
    ExitPage,
    ReferrerSource,
    ReferrerHost,
    Referrer,
    /// The organic channel of the visit's referrer (`campaign::Channel`).
    Channel,
    UtmSource,
    UtmMedium,
    UtmCampaign,
    UtmContent,
    UtmTerm,
    /// `utm_source` grouped by the campaign taxonomy: canonical, or `other`.
    UtmSourceGroup,
    /// `utm_medium` grouped by the campaign taxonomy: canonical, or `other`.
    UtmMediumGroup,
    Country,
    Region,
    City,
    DeviceType,
    Browser,
    Os,
    EventName,
}

impl Dimension {
    /// The dimensions the v1 query accepts, by their v1 names.
    pub const V1: &'static [Dimension] = &[
        Dimension::Date,
        Dimension::Hostname,
        Dimension::Path,
        Dimension::EntryPage,
        Dimension::ExitPage,
        Dimension::ReferrerSource,
        Dimension::ReferrerHost,
        Dimension::Channel,
        Dimension::UtmSource,
        Dimension::UtmMedium,
        Dimension::UtmCampaign,
        Dimension::UtmContent,
        Dimension::UtmTerm,
        Dimension::UtmSourceGroup,
        Dimension::UtmMediumGroup,
        Dimension::Country,
        Dimension::DeviceType,
        Dimension::Browser,
        Dimension::Os,
        Dimension::EventName,
    ];

    /// By its name, including the dashboard's aliases `source` and `page`.
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "date" => Self::Date,
            "hostname" => Self::Hostname,
            "page" | "path" => Self::Path,
            "entry_page" => Self::EntryPage,
            "exit_page" => Self::ExitPage,
            "source" | "referrer_source" => Self::ReferrerSource,
            "referrer_host" => Self::ReferrerHost,
            "referrer" => Self::Referrer,
            "channel" => Self::Channel,
            "utm_source" => Self::UtmSource,
            "utm_medium" => Self::UtmMedium,
            "utm_campaign" => Self::UtmCampaign,
            "utm_content" => Self::UtmContent,
            "utm_term" => Self::UtmTerm,
            "utm_source_group" => Self::UtmSourceGroup,
            "utm_medium_group" => Self::UtmMediumGroup,
            "country" => Self::Country,
            "region" => Self::Region,
            "city" => Self::City,
            "device_type" => Self::DeviceType,
            "browser" => Self::Browser,
            "os" => Self::Os,
            "event_name" => Self::EventName,
            _ => return None,
        })
    }

    /// A v1 dimension by its exact v1 name: no aliases, nothing off `V1`.
    pub fn parse_v1(name: &str) -> Option<Self> {
        Self::V1.iter().copied().find(|d| d.name() == name)
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Date => "date",
            Self::Hostname => "hostname",
            Self::Path => "path",
            Self::EntryPage => "entry_page",
            Self::ExitPage => "exit_page",
            Self::ReferrerSource => "referrer_source",
            Self::ReferrerHost => "referrer_host",
            Self::Referrer => "referrer",
            Self::Channel => "channel",
            Self::UtmSource => "utm_source",
            Self::UtmMedium => "utm_medium",
            Self::UtmCampaign => "utm_campaign",
            Self::UtmContent => "utm_content",
            Self::UtmTerm => "utm_term",
            Self::UtmSourceGroup => "utm_source_group",
            Self::UtmMediumGroup => "utm_medium_group",
            Self::Country => "country",
            Self::Region => "region",
            Self::City => "city",
            Self::DeviceType => "device_type",
            Self::Browser => "browser",
            Self::Os => "os",
            Self::EventName => "event_name",
        }
    }

    /// The column or expression on `events`, or `None` for a dimension only
    /// a session has (its entry and exit page) and for `channel`, whose
    /// expression depends on the sites queried (see [`Dimension::expr`]).
    pub fn events_expr(self) -> Option<&'static str> {
        Some(match self {
            Self::Date => "toString(date)",
            Self::Hostname => "hostname",
            Self::Path => "path",
            Self::EntryPage | Self::ExitPage | Self::Channel => return None,
            Self::ReferrerSource => "referrer_source",
            Self::ReferrerHost => "domain(referrer)",
            Self::Referrer => "referrer",
            Self::UtmSource => UTM_SOURCE.as_str(),
            Self::UtmMedium => UTM_MEDIUM.as_str(),
            Self::UtmCampaign => "utm_campaign",
            Self::UtmContent => "utm_content",
            Self::UtmTerm => "utm_term",
            Self::UtmSourceGroup => UTM_SOURCE_GROUP.as_str(),
            Self::UtmMediumGroup => UTM_MEDIUM_GROUP.as_str(),
            Self::Country => "country",
            Self::Region => "region",
            Self::City => "city",
            Self::DeviceType => "device_type",
            Self::Browser => "browser",
            Self::Os => "os",
            Self::EventName => "event_name",
        })
    }

    /// The column or expression on `sessions`, or `None` for a dimension a
    /// session does not keep (every event has its own) and for `channel`
    /// (see [`Dimension::expr`]).
    pub fn sessions_expr(self) -> Option<&'static str> {
        Some(match self {
            Self::Date => "toString(date)",
            Self::Hostname | Self::Path | Self::Region | Self::City | Self::EventName => {
                return None;
            }
            Self::Channel => return None,
            Self::EntryPage => "entry_page",
            Self::ExitPage => "exit_page",
            Self::ReferrerSource => "referrer_source",
            Self::ReferrerHost => "domain(referrer)",
            Self::Referrer => "referrer",
            Self::UtmSource => UTM_SOURCE.as_str(),
            Self::UtmMedium => UTM_MEDIUM.as_str(),
            Self::UtmCampaign => "utm_campaign",
            Self::UtmContent => "utm_content",
            Self::UtmTerm => "utm_term",
            Self::UtmSourceGroup => UTM_SOURCE_GROUP.as_str(),
            Self::UtmMediumGroup => UTM_MEDIUM_GROUP.as_str(),
            Self::Country => "country",
            Self::DeviceType => "device_type",
            Self::Browser => "browser",
            Self::Os => "os",
        })
    }

    /// The dimension's value on `table` for rows of `scopes`, with the values
    /// it binds, or `None` when the table does not keep it. Every query reads
    /// a dimension through here: `channel` is the one whose expression
    /// depends on the sites (each site's own hostnames are its `self`).
    pub fn expr(self, table: Table, scopes: &[SiteScope]) -> Option<Sql> {
        if self == Self::Channel {
            return Some(campaign::channel_sql(&REFERRER_HOST, scopes));
        }
        match table {
            Table::Events => self.events_expr(),
            Table::Sessions => self.sessions_expr(),
        }
        .map(Sql::new)
    }

    /// Whether a visit is a unit of this dimension, so that visit metrics
    /// (visits, bounce rate, visit duration) can be broken down by it. This
    /// is the v1 contract's list; `referrer` is deliberately not on it.
    pub fn is_session_level(self) -> bool {
        matches!(
            self,
            Self::Date
                | Self::EntryPage
                | Self::ExitPage
                | Self::ReferrerSource
                | Self::ReferrerHost
                | Self::Channel
                | Self::UtmSource
                | Self::UtmMedium
                | Self::UtmCampaign
                | Self::UtmContent
                | Self::UtmTerm
                | Self::UtmSourceGroup
                | Self::UtmMediumGroup
                | Self::Country
                | Self::DeviceType
                | Self::Browser
                | Self::Os
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operator {
    Is,
    IsNot,
    Contains,
    StartsWith,
}

impl Operator {
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "is" => Self::Is,
            "is_not" => Self::IsNot,
            "contains" => Self::Contains,
            "starts_with" => Self::StartsWith,
            _ => return None,
        })
    }
}

/// One condition a caller asked for. Its value is always bound.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filter {
    pub dimension: Dimension,
    pub operator: Operator,
    pub value: String,
}

impl Filter {
    /// `expr = ?` and the like, appended to `sql` with the value bound.
    /// `position()` and `startsWith()`, not LIKE: a value's `%` and `_` stay
    /// literal.
    pub fn predicate(&self, expr: &str, sql: &mut Sql) {
        self.predicate_on(&Sql::new(expr), sql);
    }

    /// [`Filter::predicate`] on an expression that binds values of its own.
    pub fn predicate_on(&self, expr: &Sql, sql: &mut Sql) {
        match self.operator {
            Operator::Is => {
                sql.append(expr).push(" = ").bind(self.value.clone());
            }
            Operator::IsNot => {
                sql.append(expr).push(" != ").bind(self.value.clone());
            }
            Operator::Contains => {
                sql.push("position(")
                    .append(expr)
                    .push(", ")
                    .bind(self.value.clone());
                sql.push(") > 0");
            }
            Operator::StartsWith => {
                sql.push("startsWith(")
                    .append(expr)
                    .push(", ")
                    .bind(self.value.clone());
                sql.push(")");
            }
        }
    }
}

/// An inclusive range of UTC days. Every date reaches the SQL as a bound
/// `YYYY-MM-DD` string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DateRange {
    pub from: chrono::NaiveDate,
    pub to: chrono::NaiveDate,
}

impl DateRange {
    pub fn new(from: chrono::NaiveDate, to: chrono::NaiveDate) -> Self {
        Self { from, to }
    }

    pub fn from_str(&self) -> String {
        self.from.to_string()
    }

    pub fn to_str(&self) -> String {
        self.to.to_string()
    }

    /// The first day a visit with an event in the range can have started on:
    /// a visit that crosses midnight has events on the day after its start.
    pub fn visits_from(&self) -> String {
        (self.from - chrono::Duration::days(1)).to_string()
    }

    /// The last day an event of a visit that started in the range can fall on.
    pub fn events_to(&self) -> String {
        (self.to + chrono::Duration::days(1)).to_string()
    }
}

/// The conditions of a query over `events`, without the `WHERE`: the sites'
/// scope, the days, `event_name = 'pageview'` when asked, and the filters. A
/// filter on a dimension only a session has (its entry or exit page) selects
/// the events of the sessions it names.
pub fn events_where(
    scopes: &[SiteScope],
    dates: &DateRange,
    filters: &[Filter],
    pageviews_only: bool,
) -> Sql {
    let mut sql = events_scope(scopes);
    sql.push(" AND date >= ").bind(dates.from_str());
    sql.push(" AND date <= ").bind(dates.to_str());
    if pageviews_only {
        sql.push(" AND event_name = 'pageview'");
    }
    for filter in filters {
        sql.push(" AND ");
        match filter.dimension.expr(Table::Events, scopes) {
            Some(expr) => filter.predicate_on(&expr, &mut sql),
            None => {
                let expr = filter
                    .dimension
                    .expr(Table::Sessions, scopes)
                    .expect("a dimension lives on at least one table");
                sql.push("session_id IN (SELECT session_id FROM sessions WHERE site_id IN (");
                sql.push(&site_ids(scopes));
                sql.push(") AND date >= ").bind(dates.visits_from());
                sql.push(" AND date <= ").bind(dates.to_str());
                sql.push(" AND ");
                filter.predicate_on(&expr, &mut sql);
                sql.push(")");
            }
        }
    }
    sql
}

/// The conditions of a query over `sessions`, without the `WHERE`. A session
/// has no hostname, so the sites' scope, and every filter on a dimension a
/// session does not keep, select by the session's events; the other filters
/// apply to the session itself.
pub fn sessions_where(scopes: &[SiteScope], dates: &DateRange, filters: &[Filter]) -> Sql {
    let mut sql = Sql::new(format!("site_id IN ({})", site_ids(scopes)));
    sql.push(" AND date >= ").bind(dates.from_str());
    sql.push(" AND date <= ").bind(dates.to_str());
    sql.push(" AND session_id IN (SELECT session_id FROM events WHERE ");
    sql.append(&events_scope(scopes));
    sql.push(" AND date >= ").bind(dates.from_str());
    sql.push(" AND date <= ").bind(dates.events_to());
    for filter in filters {
        if filter.dimension.expr(Table::Sessions, scopes).is_some() {
            continue;
        }
        let expr = filter
            .dimension
            .expr(Table::Events, scopes)
            .expect("a dimension lives on at least one table");
        sql.push(" AND ");
        filter.predicate_on(&expr, &mut sql);
    }
    sql.push(")");
    for filter in filters {
        if let Some(expr) = filter.dimension.expr(Table::Sessions, scopes) {
            sql.push(" AND ");
            filter.predicate_on(&expr, &mut sql);
        }
    }
    sql
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope(ch_id: u64) -> SiteScope {
        SiteScope {
            ch_id,
            hostnames: vec!["a.example".into(), "www.a.example".into()],
        }
    }

    fn dates() -> DateRange {
        DateRange::new(
            chrono::NaiveDate::from_ymd_opt(2026, 9, 1).unwrap(),
            chrono::NaiveDate::from_ymd_opt(2026, 9, 30).unwrap(),
        )
    }

    fn filter(dimension: &str, operator: &str, value: &str) -> Filter {
        Filter {
            dimension: Dimension::parse(dimension).unwrap(),
            operator: Operator::parse(operator).unwrap(),
            value: value.into(),
        }
    }

    #[test]
    fn every_dimension_lives_on_at_least_one_table_and_round_trips_its_name() {
        for name in [
            "date",
            "hostname",
            "path",
            "entry_page",
            "exit_page",
            "referrer_source",
            "referrer_host",
            "referrer",
            "channel",
            "utm_source",
            "utm_medium",
            "utm_campaign",
            "utm_content",
            "utm_term",
            "utm_source_group",
            "utm_medium_group",
            "country",
            "region",
            "city",
            "device_type",
            "browser",
            "os",
            "event_name",
        ] {
            let d = Dimension::parse(name).unwrap_or_else(|| panic!("{name}"));
            assert_eq!(d.name(), name);
            let scopes = [scope(7)];
            assert!(
                d.expr(Table::Events, &scopes).is_some()
                    || d.expr(Table::Sessions, &scopes).is_some(),
                "{name}"
            );
            if d.is_session_level() {
                assert!(
                    d.expr(Table::Sessions, &scopes).is_some(),
                    "{name} is session-level"
                );
            }
        }
        assert_eq!(Dimension::parse("page"), Some(Dimension::Path));
        assert_eq!(Dimension::parse("source"), Some(Dimension::ReferrerSource));
        assert_eq!(Dimension::parse("password"), None);
        assert!(
            Dimension::V1.iter().all(|d| {
                !matches!(d, Dimension::Region | Dimension::City | Dimension::Referrer)
            })
        );
    }

    #[test]
    fn the_v1_names_are_exactly_the_contract_and_take_no_alias() {
        let names: Vec<&str> = Dimension::V1.iter().map(|d| d.name()).collect();
        assert_eq!(
            names,
            [
                "date",
                "hostname",
                "path",
                "entry_page",
                "exit_page",
                "referrer_source",
                "referrer_host",
                "channel",
                "utm_source",
                "utm_medium",
                "utm_campaign",
                "utm_content",
                "utm_term",
                "utm_source_group",
                "utm_medium_group",
                "country",
                "device_type",
                "browser",
                "os",
                "event_name",
            ]
        );
        for alias in ["page", "source", "referrer", "region", "city", "Path", ""] {
            assert_eq!(Dimension::parse_v1(alias), None, "{alias:?}");
        }
        assert_eq!(
            Dimension::parse_v1("entry_page"),
            Some(Dimension::EntryPage)
        );
    }

    #[test]
    fn a_filter_value_is_bound_never_spliced() {
        let hostile = "x' OR site_id != 0 OR '1'='1";
        let sql = events_where(
            &[scope(7)],
            &dates(),
            &[filter("path", "is", hostile)],
            true,
        );
        assert!(
            !sql.text.contains(hostile),
            "the value reached the SQL: {}",
            sql.text
        );
        let without_literal = sql.text.replace("'pageview'", "");
        assert!(
            !without_literal.contains('\''),
            "no quote but the pageview literal: {without_literal}"
        );
        assert_eq!(sql.text.matches('?').count(), sql.binds.len());
        assert_eq!(sql.binds.last().map(String::as_str), Some(hostile));
        assert!(sql.text.contains("event_name = 'pageview'"));
    }

    #[test]
    fn contains_and_starts_with_are_literal_matches() {
        let sql = events_where(
            &[scope(7)],
            &dates(),
            &[
                filter("page", "contains", "50%_off"),
                filter("path", "starts_with", "/blog/"),
            ],
            false,
        );
        assert!(sql.text.contains("position(path, ?) > 0"), "{}", sql.text);
        assert!(sql.text.contains("startsWith(path, ?)"), "{}", sql.text);
        assert!(!sql.text.contains("LIKE"));
        assert!(!sql.text.contains("pageview"), "not pageviews only");
        assert_eq!(sql.binds[sql.binds.len() - 2..], ["50%_off", "/blog/"]);
    }

    #[test]
    fn several_sites_each_keep_their_own_hostnames() {
        let sql = events_where(&[scope(7), scope(9)], &dates(), &[], true);
        assert!(sql.text.starts_with("((site_id = 7 AND hostname IN (?, ?)) OR (site_id = 9 AND hostname IN (?, ?))) AND date >= ? AND date <= ?"), "{}", sql.text);
        assert_eq!(
            sql.binds,
            [
                "a.example",
                "www.a.example",
                "a.example",
                "www.a.example",
                "2026-09-01",
                "2026-09-30"
            ]
        );
    }

    #[test]
    fn a_session_only_filter_on_events_goes_through_the_sessions_it_names() {
        let sql = events_where(
            &[scope(7)],
            &dates(),
            &[filter("entry_page", "is", "/landing")],
            true,
        );
        assert!(sql.text.contains("session_id IN (SELECT session_id FROM sessions WHERE site_id IN (7) AND date >= ? AND date <= ? AND entry_page = ?)"), "{}", sql.text);
        assert_eq!(sql.text.matches('?').count(), sql.binds.len());
        // A visit that started the evening before still names its events.
        assert_eq!(
            sql.binds[sql.binds.len() - 3..],
            ["2026-08-31", "2026-09-30", "/landing"]
        );
    }

    #[test]
    fn sessions_are_scoped_through_their_events_and_filtered_on_both_tables() {
        let sql = sessions_where(
            &[scope(7)],
            &dates(),
            &[
                filter("path", "starts_with", "/blog/"),
                filter("country", "is", "DE"),
            ],
        );
        assert_eq!(
            sql.text,
            "site_id IN (7) AND date >= ? AND date <= ? AND session_id IN (SELECT session_id FROM events WHERE ((site_id = 7 AND hostname IN (?, ?))) AND date >= ? AND date <= ? AND startsWith(path, ?)) AND country = ?"
        );
        assert_eq!(
            sql.binds,
            [
                "2026-09-01",
                "2026-09-30",
                "a.example",
                "www.a.example",
                "2026-09-01",
                "2026-10-01",
                "/blog/",
                "DE"
            ]
        );
    }

    #[test]
    fn a_channel_filter_binds_each_sites_own_hostnames_on_both_tables() {
        let filters = [filter("channel", "is", "search")];
        let events = events_where(&[scope(7)], &dates(), &filters, true);
        assert_eq!(events.text.matches('?').count(), events.binds.len());
        assert!(
            events.text.contains("(site_id = 7 AND (0 OR h = ?"),
            "{}",
            events.text
        );
        assert_eq!(events.binds.last().map(String::as_str), Some("search"));
        assert!(
            events.binds.contains(&".www.a.example".to_string()),
            "{:?}",
            events.binds
        );

        let sessions = sessions_where(&[scope(7)], &dates(), &filters);
        assert_eq!(sessions.text.matches('?').count(), sessions.binds.len());
        // Once, on the session's own referrer, not through its events.
        assert_eq!(
            sessions.text.matches("arrayMap(h ->").count(),
            1,
            "{}",
            sessions.text
        );
        assert!(
            sessions
                .text
                .contains(") AND arrayElement(arrayMap(h -> multiIf("),
            "{}",
            sessions.text
        );
        assert!(
            sessions.text.contains("extractGroups(referrer, "),
            "{}",
            sessions.text
        );
        assert_eq!(sessions.binds.last().map(String::as_str), Some("search"));
    }

    #[test]
    fn the_campaign_columns_are_read_normalised_on_both_tables() {
        for table in [Table::Events, Table::Sessions] {
            let source = Dimension::UtmSource.expr(table, &[]).unwrap();
            assert!(
                source.text.starts_with("transform(lowerUTF8("),
                "{}",
                source.text
            );
            assert!(source.binds.is_empty());
            let group = Dimension::UtmMediumGroup.expr(table, &[]).unwrap();
            assert!(group.text.ends_with(", 'other')"), "{}", group.text);
            // Content, campaign and term are free text: as stored.
            assert_eq!(
                Dimension::UtmCampaign.expr(table, &[]).unwrap().text,
                "utm_campaign"
            );
        }
    }

    #[test]
    fn the_buffer_keeps_binds_in_textual_order() {
        let mut sql = Sql::new("a = ");
        sql.bind("1")
            .push(" AND b IN ")
            .bind_list(&["x".into(), "y".into()]);
        let mut tail = Sql::new(" AND c = ");
        tail.bind("z");
        sql.append(&tail);
        assert_eq!(sql.text, "a = ? AND b IN (?, ?) AND c = ?");
        assert_eq!(sql.binds, ["1", "x", "y", "z"]);
    }
}
