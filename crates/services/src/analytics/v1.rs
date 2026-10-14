//! The v1 query (`POST /api/v1/query`): one site broken down by up to four
//! dimensions, with visitor, visit, pageview and reading metrics and the
//! goals completed, per row. `docs/api.md` is the contract. The dashboard
//! runs the same query for one site or several (`query_v1_sites`), so the
//! numbers on screen are the numbers an API key reads.
//!
//! Every metric is computed over the sites' own hostnames (`SiteScope`), in
//! one ClickHouse query: a family of rows per kind of fact, joined by their
//! dimension values and sorted and paged there.
//!
//! - **events**: `pageview` events give visitors and pageviews; a goal's
//!   events give the visitors who completed it.
//! - **reads**: `engagement` events, reduced to one engaged time per
//!   pageview (the longest report carrying its `pv` id; a report without
//!   one stands for its own pageview).
//! - **sessions**: visits, bounces and visit duration, only when every
//!   dimension is one a visit has.
//!
//! A visit attribute (referrer, campaign, entry and exit page) is the visit's
//! for every event of the visit, so a goal completed three pages after the
//! landing counts for the landing's campaign. A visit still in progress has
//! no row in `sessions` yet; its events count with their own referrer and
//! campaign until it has.

use std::collections::HashSet;

use purestat_db::models::{Goal, GoalType};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::campaign;
use super::query::{QueryError, QueryService, parse_date};
use super::scope::{SiteScope, events_scope, site_ids};
use super::sql::{DateRange, Dimension, Filter, Operator, Sql, Table, sessions_where};

pub const MAX_DIMENSIONS: usize = 4;
pub const MAX_GOALS: usize = 10;
pub const DEFAULT_LIMIT: u64 = 100;
pub const MAX_LIMIT: u64 = 1000;
/// The furthest `date_to` may be from `date_from`, in days.
pub const MAX_SPAN_DAYS: i64 = 366;
/// A pageview is a read once the page was visible for this many seconds.
pub const READ_SECONDS: u32 = 30;

/// The request body, as sent. Unknown fields are refused, so a misspelled
/// one is an error rather than a silently unfiltered answer.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct V1Request {
    pub date_from: String,
    pub date_to: String,
    pub dimensions: Option<Vec<String>>,
    pub metrics: Vec<String>,
    pub goals: Option<Vec<String>>,
    pub filters: Option<Vec<V1Filter>>,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct V1Filter {
    pub dimension: String,
    pub operator: String,
    pub value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Metric {
    Visitors,
    Visits,
    Pageviews,
    BounceRate,
    VisitDuration,
    Reads,
    MedianReadS,
}

impl Metric {
    pub const ALL: [Metric; 7] = [
        Metric::Visitors,
        Metric::Visits,
        Metric::Pageviews,
        Metric::BounceRate,
        Metric::VisitDuration,
        Metric::Reads,
        Metric::MedianReadS,
    ];

    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|m| m.name() == name)
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Visitors => "visitors",
            Self::Visits => "visits",
            Self::Pageviews => "pageviews",
            Self::BounceRate => "bounce_rate",
            Self::VisitDuration => "visit_duration",
            Self::Reads => "reads",
            Self::MedianReadS => "median_read_s",
        }
    }

    fn of_visits(self) -> bool {
        matches!(self, Self::Visits | Self::BounceRate | Self::VisitDuration)
    }

    fn of_reads(self) -> bool {
        matches!(self, Self::Reads | Self::MedianReadS)
    }

    fn of_pageviews(self) -> bool {
        matches!(self, Self::Visitors | Self::Pageviews)
    }
}

/// What completes a goal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GoalCondition {
    /// A custom event of this name.
    Event(String),
    /// A pageview of this path.
    Page(String),
}

/// A goal the caller asked for, by its name. Several goals of one site can
/// share a name; completing any of them completes the name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoalSpec {
    pub name: String,
    pub conditions: Vec<GoalCondition>,
}

/// A request that passed every check: what the SQL is built from.
#[derive(Debug, Clone)]
pub struct V1Query {
    pub dates: DateRange,
    pub dimensions: Vec<Dimension>,
    pub metrics: Vec<Metric>,
    pub goals: Vec<GoalSpec>,
    pub filters: Vec<Filter>,
    pub limit: u64,
    pub offset: u64,
}

impl V1Query {
    /// Checks the request against the contract and the site's goals. The
    /// error is the message a 400 carries.
    pub fn parse(req: V1Request, site_goals: &[Goal]) -> Result<Self, String> {
        let date = |field: &str, value: &str| {
            parse_date(value)
                .map_err(|_| format!("{field} must be a date as YYYY-MM-DD, not {value:?}"))
        };
        let from = date("date_from", &req.date_from)?;
        let to = date("date_to", &req.date_to)?;
        if to < from {
            return Err("date_to is before date_from".to_string());
        }
        if (to - from).num_days() > MAX_SPAN_DAYS {
            return Err(format!("the dates span more than {MAX_SPAN_DAYS} days"));
        }

        let names = req.dimensions.unwrap_or_default();
        if names.len() > MAX_DIMENSIONS {
            return Err(format!("at most {MAX_DIMENSIONS} dimensions"));
        }
        let dimensions = unique("dimension", &names, Dimension::parse_v1)?;

        if req.metrics.is_empty() {
            return Err("metrics must name at least one metric".to_string());
        }
        let metrics = unique("metric", &req.metrics, Metric::parse)?;

        let goal_names = req.goals.unwrap_or_default();
        if goal_names.len() > MAX_GOALS {
            return Err(format!("at most {MAX_GOALS} goals"));
        }
        unique("goal", &goal_names, |n| Some(n.to_string()))?;
        let goals = goal_names
            .iter()
            .map(|name| {
                let defined: Vec<&Goal> = site_goals.iter().filter(|g| g.name == *name).collect();
                if defined.is_empty() {
                    return Err(format!("unknown goal: {name:?}"));
                }
                let conditions = defined
                    .into_iter()
                    .filter_map(|g| match g.goal_type {
                        GoalType::CustomEvent => g.event_name.clone().map(GoalCondition::Event),
                        GoalType::Pageview => g.page_path.clone().map(GoalCondition::Page),
                    })
                    .collect();
                Ok(GoalSpec {
                    name: name.clone(),
                    conditions,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;

        let filters = req
            .filters
            .unwrap_or_default()
            .into_iter()
            .map(|f| {
                let dimension = Dimension::parse_v1(&f.dimension)
                    .ok_or_else(|| format!("unknown filter dimension: {:?}", f.dimension))?;
                let operator = Operator::parse(&f.operator)
                    .ok_or_else(|| format!("unknown filter operator: {:?}", f.operator))?;
                Ok(Filter {
                    dimension,
                    operator,
                    value: f.value,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;

        let limit = req.limit.unwrap_or(DEFAULT_LIMIT);
        if !(1..=MAX_LIMIT).contains(&limit) {
            return Err(format!("limit must be between 1 and {MAX_LIMIT}"));
        }

        Ok(Self {
            dates: DateRange::new(from, to),
            dimensions,
            metrics,
            goals,
            filters,
            limit,
            offset: req.offset.unwrap_or(0),
        })
    }
}

/// Each name parsed, and none twice.
fn unique<T>(
    what: &str,
    names: &[String],
    parse: impl Fn(&str) -> Option<T>,
) -> Result<Vec<T>, String> {
    let mut seen = HashSet::new();
    names
        .iter()
        .map(|name| {
            if !seen.insert(name.as_str()) {
                return Err(format!("{what} named twice: {name:?}"));
            }
            parse(name).ok_or_else(|| format!("unknown {what}: {name:?}"))
        })
        .collect()
}

/// One row of the answer.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct V1Row {
    pub dimensions: Map<String, Value>,
    pub metrics: Map<String, Value>,
    /// Per goal, the distinct visitors who completed it.
    pub goals: Map<String, Value>,
    /// Per goal, those visitors as a share of the row's `visitors`, from 0
    /// to 1; `null` in a row without visitors. Present when goals are asked.
    #[serde(skip_serializing_if = "Map::is_empty")]
    pub conversion_rates: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct V1Result {
    pub rows: Vec<V1Row>,
    pub has_more: bool,
}

/// Which families of rows a query needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Plan {
    /// Count pageview events (visitors, pageviews).
    pageviews: bool,
    /// Read events at all: pageviews, or goals.
    events: bool,
    reads: bool,
    sessions: bool,
    /// Join each event to its visit, for a visit attribute.
    visit_join: bool,
}

impl Plan {
    fn of(q: &V1Query) -> Self {
        let wants = |of: fn(Metric) -> bool| q.metrics.iter().any(|m| of(*m));
        let sessions =
            wants(Metric::of_visits) && q.dimensions.iter().all(|d| d.is_session_level());
        let reads = wants(Metric::of_reads);
        // A goal's conversion rate is over the row's visitors, so goals count
        // pageviews too. With nothing else to give it rows, a breakdown lists
        // the values pageviews have, so a metric that is null for it still
        // has rows.
        let pageviews = wants(Metric::of_pageviews) || !q.goals.is_empty() || (!sessions && !reads);
        let events = pageviews || !q.goals.is_empty();
        let visit_join = (events || reads)
            && q.dimensions
                .iter()
                .chain(q.filters.iter().map(|f| &f.dimension))
                .any(|d| from_visit(*d));
        Self {
            pageviews,
            events,
            reads,
            sessions,
            visit_join,
        }
    }
}

/// A dimension an event takes from its visit rather than from itself.
fn from_visit(d: Dimension) -> bool {
    use Dimension::*;
    matches!(
        d,
        Referrer
            | ReferrerSource
            | ReferrerHost
            | Channel
            | UtmSource
            | UtmMedium
            | UtmCampaign
            | UtmContent
            | UtmTerm
            | UtmSourceGroup
            | UtmMediumGroup
            | EntryPage
            | ExitPage
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Family {
    Events,
    Reads,
}

/// A visit attribute of an event: the joined visit's (`v_*`), or the
/// event's own while its visit has no row yet.
fn visit_attr(column: &str) -> String {
    format!("if(v_session_id != '', v_{column}, {column})")
}

/// A dimension's value for an event of one of `scopes`. The campaign
/// columns are normalised and grouped by the campaign taxonomy, and the
/// channel is read from the visit's referrer (`campaign`).
fn event_expr(d: Dimension, family: Family, scopes: &[SiteScope]) -> Sql {
    use Dimension::*;
    let text = match d {
        Date => "toString(date)".to_string(),
        Hostname => "hostname".to_string(),
        Path => "path".to_string(),
        // A read is a pageview's: it counts under the pageview's name.
        EventName if family == Family::Reads => "'pageview'".to_string(),
        EventName => "event_name".to_string(),
        Country => "country".to_string(),
        Region => "region".to_string(),
        City => "city".to_string(),
        DeviceType => "device_type".to_string(),
        Browser => "browser".to_string(),
        Os => "os".to_string(),
        Referrer => visit_attr("referrer"),
        ReferrerSource => visit_attr("referrer_source"),
        ReferrerHost => format!("domain({})", visit_attr("referrer")),
        Channel => {
            let host = campaign::referrer_host_sql(&visit_attr("referrer"));
            return campaign::channel_sql(&host, scopes);
        }
        UtmSource => campaign::normalize_source_sql(&visit_attr("utm_source")),
        UtmMedium => campaign::normalize_medium_sql(&visit_attr("utm_medium")),
        UtmCampaign => visit_attr("utm_campaign"),
        UtmContent => visit_attr("utm_content"),
        UtmTerm => visit_attr("utm_term"),
        UtmSourceGroup => campaign::source_group_sql(&visit_attr("utm_source")),
        UtmMediumGroup => campaign::medium_group_sql(&visit_attr("utm_medium")),
        EntryPage => "v_entry_page".to_string(),
        ExitPage => "v_exit_page".to_string(),
    };
    Sql::new(text)
}

/// `CAST(<d's value> AS String) AS <alias>, `
fn push_dimension(sql: &mut Sql, d: Dimension, family: Family, scopes: &[SiteScope], alias: &str) {
    sql.push("CAST(")
        .append(&event_expr(d, family, scopes))
        .push(&format!(" AS String) AS {alias}, "));
}

/// `LEFT JOIN` each event to its visit's attributes. One row per visit,
/// from visits that started up to a day before the range.
fn visit_join(scopes: &[SiteScope], dates: &DateRange) -> Sql {
    let mut sql = Sql::new(format!(
        " LEFT JOIN (SELECT session_id AS v_session_id, \
         any(entry_page) AS v_entry_page, any(exit_page) AS v_exit_page, \
         any(referrer) AS v_referrer, any(referrer_source) AS v_referrer_source, \
         any(utm_source) AS v_utm_source, any(utm_medium) AS v_utm_medium, \
         any(utm_campaign) AS v_utm_campaign, any(utm_content) AS v_utm_content, \
         any(utm_term) AS v_utm_term \
         FROM sessions WHERE site_id IN ({}) AND date >= ",
        site_ids(scopes)
    ));
    sql.bind(dates.visits_from());
    sql.push(" AND date <= ").bind(dates.to_str());
    sql.push(" GROUP BY session_id) AS v ON session_id = v_session_id");
    sql
}

/// `FROM events [JOIN] WHERE <the sites' hostnames and the days>`.
fn from_events(scopes: &[SiteScope], dates: &DateRange, plan: &Plan) -> Sql {
    let mut sql = Sql::new(" FROM events");
    if plan.visit_join {
        sql.append(&visit_join(scopes, dates));
    }
    sql.push(" WHERE ");
    sql.append(&events_scope(scopes));
    sql.push(" AND date >= ").bind(dates.from_str());
    sql.push(" AND date <= ").bind(dates.to_str());
    sql
}

fn push_event_filters(sql: &mut Sql, filters: &[Filter], family: Family, scopes: &[SiteScope]) {
    for filter in filters {
        sql.push(" AND ");
        filter.predicate_on(&event_expr(filter.dimension, family, scopes), sql);
    }
}

fn push_goal_condition(sql: &mut Sql, goal: &GoalSpec) {
    if goal.conditions.is_empty() {
        sql.push("0");
        return;
    }
    sql.push("(");
    for (i, condition) in goal.conditions.iter().enumerate() {
        if i > 0 {
            sql.push(" OR ");
        }
        match condition {
            GoalCondition::Event(name) => {
                sql.push("event_name = ").bind(name.clone());
            }
            GoalCondition::Page(path) => {
                sql.push("(event_name = 'pageview' AND path = ")
                    .bind(path.clone());
                sql.push(")");
            }
        }
    }
    sql.push(")");
}

fn push_group_by(sql: &mut Sql, dimensions: usize) {
    if dimensions > 0 {
        let keys: Vec<String> = (0..dimensions).map(|i| format!("d{i}")).collect();
        sql.push(&format!(" GROUP BY {}", keys.join(", ")));
    }
}

/// The zero columns of the metrics a family does not compute.
fn zeros(names: &[&str]) -> String {
    names
        .iter()
        .map(|n| {
            if *n == "f_median" {
                "toFloat64(0) AS f_median".to_string()
            } else {
                format!("toUInt64(0) AS {n}")
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn goal_zeros(goals: usize) -> String {
    (0..goals)
        .map(|i| format!(", toUInt64(0) AS f_g{i}"))
        .collect()
}

/// Pageviews and goal completions per row.
fn events_family(q: &V1Query, scopes: &[SiteScope], plan: &Plan) -> Sql {
    let mut sql = Sql::new("SELECT ");
    for (i, d) in q.dimensions.iter().enumerate() {
        push_dimension(&mut sql, *d, Family::Events, scopes, &format!("d{i}"));
    }
    sql.push(
        "uniqExactIf(visitor_hash, event_name = 'pageview') AS f_visitors, \
         countIf(event_name = 'pageview') AS f_pageviews",
    );
    for (i, goal) in q.goals.iter().enumerate() {
        sql.push(", uniqExactIf(visitor_hash, ");
        push_goal_condition(&mut sql, goal);
        sql.push(&format!(") AS f_g{i}"));
    }
    sql.push(", ");
    sql.push(&zeros(&[
        "f_visits",
        "f_bounces",
        "f_duration",
        "f_reads",
        "f_engaged",
        "f_median",
    ]));
    sql.append(&from_events(scopes, &q.dates, plan));
    // Only the events some metric counts.
    let mut counted = Vec::new();
    if plan.pageviews {
        counted.push(Sql::new("event_name = 'pageview'"));
    }
    for goal in &q.goals {
        let mut condition = Sql::default();
        push_goal_condition(&mut condition, goal);
        counted.push(condition);
    }
    sql.push(" AND (");
    for (i, condition) in counted.iter().enumerate() {
        if i > 0 {
            sql.push(" OR ");
        }
        sql.append(condition);
    }
    sql.push(")");
    push_event_filters(&mut sql, &q.filters, Family::Events, scopes);
    push_group_by(&mut sql, q.dimensions.len());
    sql
}

/// Reads and the median engaged time per row, over engaged pageviews.
fn reads_family(q: &V1Query, scopes: &[SiteScope], plan: &Plan) -> Sql {
    let n = q.dimensions.len();
    // Every engagement report: its dimensions, time, pageview id and seconds.
    let mut reports = Sql::new("SELECT ");
    for (i, d) in q.dimensions.iter().enumerate() {
        push_dimension(&mut reports, *d, Family::Reads, scopes, &format!("p{i}"));
    }
    reports.push(
        "timestamp AS t, \
         if(has(prop_keys, 'pv'), prop_values[indexOf(prop_keys, 'pv')], '') AS pv, \
         toFloat64OrZero(if(has(prop_keys, 'duration'), prop_values[indexOf(prop_keys, 'duration')], '')) AS dur",
    );
    reports.append(&from_events(scopes, &q.dates, plan));
    reports.push(" AND event_name = 'engagement'");
    push_event_filters(&mut reports, &q.filters, Family::Reads, scopes);

    let mut sql = Sql::new("SELECT ");
    for i in 0..n {
        sql.push(&format!("d{i}, "));
    }
    sql.push(&zeros(&["f_visitors", "f_pageviews"]));
    sql.push(&goal_zeros(q.goals.len()));
    sql.push(", ");
    sql.push(&zeros(&["f_visits", "f_bounces", "f_duration"]));
    sql.push(&format!(
        ", countIf(engaged >= {READ_SECONDS}) AS f_reads, count() AS f_engaged, \
         quantileExactInclusive(0.5)(engaged) AS f_median FROM ("
    ));
    // A pageview with an id is engaged for its longest report...
    sql.push("SELECT ");
    for i in 0..n {
        sql.push(&format!("argMin(p{i}, t) AS d{i}, "));
    }
    sql.push("max(dur) AS engaged FROM (");
    sql.append(&reports);
    sql.push(") WHERE pv != '' AND dur > 0 AND isFinite(dur) GROUP BY pv UNION ALL SELECT ");
    // ...and a report without one stands for its own pageview.
    for i in 0..n {
        sql.push(&format!("p{i} AS d{i}, "));
    }
    sql.push("dur AS engaged FROM (");
    sql.append(&reports);
    sql.push(") WHERE pv = '' AND dur > 0 AND isFinite(dur))");
    push_group_by(&mut sql, n);
    sql
}

/// Visits, bounces and their total duration per row.
fn sessions_family(q: &V1Query, scopes: &[SiteScope]) -> Sql {
    let mut sql = Sql::new("SELECT ");
    for (i, d) in q.dimensions.iter().enumerate() {
        let expr = d
            .expr(Table::Sessions, scopes)
            .expect("a session-level dimension is on sessions");
        sql.push("CAST(")
            .append(&expr)
            .push(&format!(" AS String) AS d{i}, "));
    }
    sql.push(&zeros(&["f_visitors", "f_pageviews"]));
    sql.push(&goal_zeros(q.goals.len()));
    sql.push(
        ", count() AS f_visits, countIf(is_bounce = 1) AS f_bounces, \
         sum(toUInt64(duration)) AS f_duration, ",
    );
    sql.push(&zeros(&["f_reads", "f_engaged", "f_median"]));
    sql.push(" FROM sessions WHERE ");
    sql.append(&sessions_where(scopes, &q.dates, &q.filters));
    push_group_by(&mut sql, q.dimensions.len());
    sql
}

/// What the rows are sorted by: the first metric, when the query computes it.
fn sort_expr(metric: Metric, plan: &Plan) -> Option<&'static str> {
    match metric {
        Metric::Visitors => Some("visitors"),
        Metric::Pageviews => Some("pageviews"),
        Metric::Reads => Some("reads"),
        Metric::MedianReadS => Some("if(engaged_n = 0, NULL, median_read)"),
        Metric::Visits if plan.sessions => Some("visits"),
        Metric::BounceRate if plan.sessions => Some("if(visits = 0, NULL, bounces / visits)"),
        Metric::VisitDuration if plan.sessions => {
            Some("if(visits = 0, NULL, duration_sum / visits)")
        }
        Metric::Visits | Metric::BounceRate | Metric::VisitDuration => None,
    }
}

fn build(q: &V1Query, scopes: &[SiteScope]) -> (Sql, Plan) {
    let plan = Plan::of(q);
    let n = q.dimensions.len();
    let dims: Vec<String> = (0..n).map(|i| format!("d{i}")).collect();

    let mut sql = Sql::new("SELECT ");
    if n == 0 {
        sql.push("CAST([] AS Array(String)) AS dims");
    } else {
        sql.push(&format!("[{}] AS dims", dims.join(", ")));
    }
    sql.push(", sum(f_visitors) AS visitors, sum(f_pageviews) AS pageviews, ");
    if q.goals.is_empty() {
        sql.push("CAST([] AS Array(UInt64)) AS goals");
    } else {
        let sums: Vec<String> = (0..q.goals.len()).map(|i| format!("sum(f_g{i})")).collect();
        sql.push(&format!("[{}] AS goals", sums.join(", ")));
    }
    sql.push(
        ", sum(f_visits) AS visits, sum(f_bounces) AS bounces, sum(f_duration) AS duration_sum, \
         sum(f_reads) AS reads, sum(f_engaged) AS engaged_n, max(f_median) AS median_read FROM (",
    );
    let mut families = Vec::new();
    if plan.events {
        families.push(events_family(q, scopes, &plan));
    }
    if plan.reads {
        families.push(reads_family(q, scopes, &plan));
    }
    if plan.sessions {
        families.push(sessions_family(q, scopes));
    }
    for (i, family) in families.iter().enumerate() {
        if i > 0 {
            sql.push(" UNION ALL ");
        }
        sql.append(family);
    }
    sql.push(")");
    push_group_by(&mut sql, n);

    let mut order: Vec<String> = Vec::new();
    if let Some(expr) = sort_expr(q.metrics[0], &plan) {
        order.push(format!("{expr} DESC NULLS LAST"));
    }
    order.extend(dims.iter().map(|d| format!("{d} ASC")));
    if !order.is_empty() {
        sql.push(&format!(" ORDER BY {}", order.join(", ")));
    }
    // One row past the page says whether there is another page.
    sql.push(&format!(" LIMIT {} OFFSET {}", q.limit + 1, q.offset));
    (sql, plan)
}

#[derive(Debug, clickhouse::Row, Deserialize)]
struct RawRow {
    dims: Vec<String>,
    visitors: u64,
    pageviews: u64,
    goals: Vec<u64>,
    visits: u64,
    bounces: u64,
    duration_sum: u64,
    reads: u64,
    engaged_n: u64,
    median_read: f64,
}

fn shape(q: &V1Query, plan: &Plan, raw: RawRow) -> V1Row {
    let dimensions = q
        .dimensions
        .iter()
        .zip(raw.dims)
        .map(|(d, v)| (d.name().to_string(), Value::from(v)))
        .collect();
    let per_visit = |total: u64| {
        if plan.sessions && raw.visits > 0 {
            Value::from(total as f64 / raw.visits as f64)
        } else {
            Value::Null
        }
    };
    let metrics = q
        .metrics
        .iter()
        .map(|m| {
            let value = match m {
                Metric::Visitors => Value::from(raw.visitors),
                Metric::Pageviews => Value::from(raw.pageviews),
                Metric::Visits if plan.sessions => Value::from(raw.visits),
                Metric::Visits => Value::Null,
                Metric::BounceRate => per_visit(raw.bounces),
                Metric::VisitDuration => per_visit(raw.duration_sum),
                Metric::Reads => Value::from(raw.reads),
                Metric::MedianReadS if raw.engaged_n > 0 => Value::from(raw.median_read),
                Metric::MedianReadS => Value::Null,
            };
            (m.name().to_string(), value)
        })
        .collect();
    let goals = q
        .goals
        .iter()
        .zip(&raw.goals)
        .map(|(g, v)| (g.name.clone(), Value::from(*v)))
        .collect();
    let conversion_rates = q
        .goals
        .iter()
        .zip(&raw.goals)
        .map(|(g, v)| (g.name.clone(), share(*v, raw.visitors)))
        .collect();
    V1Row {
        dimensions,
        metrics,
        goals,
        conversion_rates,
    }
}

/// `part / whole` from 0 to 1, or `null` without a whole. A visitor whose
/// visit crossed midnight UTC can complete a goal on a day without a
/// pageview of theirs, so a share is capped at 1.
fn share(part: u64, whole: u64) -> Value {
    if whole == 0 {
        Value::Null
    } else {
        Value::from((part as f64 / whole as f64).min(1.0))
    }
}

impl QueryService {
    /// Runs a checked v1 query over the site's scope.
    pub async fn query_v1(&self, scope: &SiteScope, q: &V1Query) -> Result<V1Result, QueryError> {
        self.query_v1_sites(std::slice::from_ref(scope), q).await
    }

    /// Runs a checked v1 query over several sites of one org at once, as the
    /// dashboard does for its selected sites. The route resolves every site
    /// within the caller's org before it gets here.
    pub async fn query_v1_sites(
        &self,
        scopes: &[SiteScope],
        q: &V1Query,
    ) -> Result<V1Result, QueryError> {
        if scopes.is_empty() {
            return Err(QueryError::InvalidQuery("No site IDs provided".to_string()));
        }
        let (sql, plan) = build(q, scopes);
        let raw = sql.query(self.client()).fetch_all::<RawRow>().await?;
        let has_more = raw.len() as u64 > q.limit;
        let rows = raw
            .into_iter()
            .take(q.limit as usize)
            .map(|r| shape(q, &plan, r))
            .collect();
        Ok(V1Result { rows, has_more })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bson::{DateTime, oid::ObjectId};

    fn goal(name: &str, goal_type: GoalType, event: Option<&str>, page: Option<&str>) -> Goal {
        Goal {
            id: Some(ObjectId::new()),
            site_id: ObjectId::new(),
            org_id: ObjectId::new(),
            goal_type,
            name: name.into(),
            event_name: event.map(String::from),
            page_path: page.map(String::from),
            created_at: DateTime::now(),
        }
    }

    fn goals() -> Vec<Goal> {
        vec![
            goal("signup", GoalType::CustomEvent, Some("signup"), None),
            goal("thanks", GoalType::Pageview, None, Some("/thanks")),
        ]
    }

    fn request(body: serde_json::Value) -> V1Request {
        serde_json::from_value(body).expect("a request body")
    }

    fn parse(body: serde_json::Value) -> Result<V1Query, String> {
        V1Query::parse(request(body), &goals())
    }

    fn scope() -> SiteScope {
        SiteScope {
            ch_id: 7,
            hostnames: vec!["a.example".into(), "www.a.example".into()],
        }
    }

    fn base() -> serde_json::Value {
        serde_json::json!({ "date_from": "2026-09-01", "date_to": "2026-09-30", "metrics": ["visitors"] })
    }

    fn with(mut body: serde_json::Value, key: &str, value: serde_json::Value) -> serde_json::Value {
        body[key] = value;
        body
    }

    #[test]
    fn a_minimal_request_takes_the_defaults() {
        let q = parse(base()).unwrap();
        assert_eq!(q.limit, 100);
        assert_eq!(q.offset, 0);
        assert!(q.dimensions.is_empty() && q.goals.is_empty() && q.filters.is_empty());
        assert_eq!(q.metrics, [Metric::Visitors]);
    }

    #[test]
    fn dates_are_checked() {
        let err = |from: &str, to: &str| {
            parse(with(
                with(base(), "date_from", from.into()),
                "date_to",
                to.into(),
            ))
            .unwrap_err()
        };
        assert!(err("2026-09-30", "2026-09-01").contains("before"));
        assert!(
            err("2025-01-01", "2026-02-05").contains("366 days"),
            "a 400-day span"
        );
        assert!(err("2026-09-01' OR '1'='1", "2026-09-30").contains("date_from"));
        assert!(
            parse(with(
                with(base(), "date_from", "2025-09-01".into()),
                "date_to",
                "2026-09-02".into()
            ))
            .is_ok(),
            "366 days apart"
        );
        assert!(
            parse(with(
                with(base(), "date_from", "2026-09-01".into()),
                "date_to",
                "2026-09-01".into()
            ))
            .is_ok(),
            "one day"
        );
    }

    #[test]
    fn names_are_checked() {
        let bad =
            |key: &str, value: serde_json::Value| parse(with(base(), key, value)).unwrap_err();
        assert!(
            bad("metrics", serde_json::json!(["visitors", "revenue"])).contains("unknown metric")
        );
        assert!(bad("metrics", serde_json::json!([])).contains("at least one"));
        assert!(bad("metrics", serde_json::json!(["visitors", "visitors"])).contains("twice"));
        assert!(bad("dimensions", serde_json::json!(["page"])).contains("unknown dimension"));
        assert!(
            bad(
                "dimensions",
                serde_json::json!(["date", "path", "os", "browser", "country"])
            )
            .contains("at most 4")
        );
        assert!(bad("goals", serde_json::json!(["purchase"])).contains("unknown goal"));
        assert!(
            bad(
                "filters",
                serde_json::json!([{ "dimension": "path", "operator": "matches", "value": "x" }])
            )
            .contains("operator")
        );
        assert!(
            bad(
                "filters",
                serde_json::json!([{ "dimension": "city", "operator": "is", "value": "x" }])
            )
            .contains("filter dimension")
        );
        assert!(bad("limit", serde_json::json!(0)).contains("limit"));
        assert!(bad("limit", serde_json::json!(1001)).contains("limit"));
        assert!(
            serde_json::from_value::<V1Request>(with(base(), "site_id", "x".into())).is_err(),
            "unknown fields are refused"
        );
    }

    #[test]
    fn goals_resolve_to_their_definitions() {
        let q = parse(with(
            base(),
            "goals",
            serde_json::json!(["thanks", "signup"]),
        ))
        .unwrap();
        assert_eq!(
            q.goals[0].conditions,
            [GoalCondition::Page("/thanks".into())]
        );
        assert_eq!(
            q.goals[1].conditions,
            [GoalCondition::Event("signup".into())]
        );
    }

    fn sql_of(body: serde_json::Value) -> (Sql, Plan) {
        build(&parse(body).unwrap(), &[scope()])
    }

    #[test]
    fn every_value_is_bound_and_every_placeholder_has_one() {
        let hostile = "x' OR 1=1 OR '";
        let (sql, _) = sql_of(serde_json::json!({
            "date_from": "2026-09-01", "date_to": "2026-09-30",
            "dimensions": ["utm_source", "path"],
            "metrics": ["visitors", "visits", "reads", "median_read_s"],
            "goals": ["signup", "thanks"],
            "filters": [
                { "dimension": "path", "operator": "starts_with", "value": hostile },
                { "dimension": "entry_page", "operator": "is", "value": hostile },
            ],
        }));
        assert!(!sql.text.contains(hostile), "{}", sql.text);
        assert_eq!(
            sql.text.matches('?').count(),
            sql.binds.len(),
            "{}",
            sql.text
        );
        assert!(sql.binds.iter().filter(|b| *b == hostile).count() >= 4);
    }

    #[test]
    fn visit_metrics_are_computed_only_for_visit_dimensions() {
        let (_, plan) = sql_of(serde_json::json!({
            "date_from": "2026-09-01", "date_to": "2026-09-30",
            "dimensions": ["path"], "metrics": ["visits"],
        }));
        assert!(!plan.sessions, "a path is not something a visit has");
        assert!(plan.pageviews, "the rows still list the paths");

        let (sql, plan) = sql_of(serde_json::json!({
            "date_from": "2026-09-01", "date_to": "2026-09-30",
            "dimensions": ["date", "utm_source"], "metrics": ["visits", "visitors"],
        }));
        assert!(plan.sessions && plan.events && plan.visit_join);
        assert!(
            sql.text.contains(" FROM sessions WHERE site_id IN (7)"),
            "{}",
            sql.text
        );
        assert!(
            sql.text
                .contains("ORDER BY visits DESC NULLS LAST, d0 ASC, d1 ASC LIMIT 101 OFFSET 0"),
            "{}",
            sql.text
        );
    }

    #[test]
    fn a_campaign_is_the_visits_and_a_page_is_the_events_own() {
        let (sql, plan) = sql_of(serde_json::json!({
            "date_from": "2026-09-01", "date_to": "2026-09-30",
            "dimensions": ["utm_source", "path"], "metrics": ["pageviews"],
        }));
        assert!(plan.visit_join);
        // The visit's source, normalised by the campaign taxonomy.
        assert!(
            sql.text
                .contains("CAST(transform(lowerUTF8(replaceRegexpAll(if(v_session_id != '', v_utm_source, utm_source), "),
            "{}",
            sql.text
        );
        assert!(
            sql.text.contains("CAST(path AS String) AS d1"),
            "{}",
            sql.text
        );
        // Visits that started the day before the range still lend their attributes.
        assert!(sql.binds.contains(&"2026-08-31".to_string()));

        let (sql, plan) = sql_of(serde_json::json!({
            "date_from": "2026-09-01", "date_to": "2026-09-30",
            "dimensions": ["path"], "metrics": ["pageviews"],
        }));
        assert!(
            !plan.visit_join && !sql.text.contains("JOIN"),
            "{}",
            sql.text
        );
    }

    #[test]
    fn rows_are_shaped_with_nulls_where_a_metric_has_no_value() {
        let q = parse(serde_json::json!({
            "date_from": "2026-09-01", "date_to": "2026-09-30",
            "dimensions": ["utm_source"],
            "metrics": ["visitors", "visits", "bounce_rate", "visit_duration", "median_read_s"],
            "goals": ["signup"],
        }))
        .unwrap();
        let plan = Plan::of(&q);
        let row = shape(
            &q,
            &plan,
            RawRow {
                dims: vec!["linkedin".into()],
                visitors: 3,
                pageviews: 9,
                goals: vec![1],
                visits: 4,
                bounces: 1,
                duration_sum: 200,
                reads: 0,
                engaged_n: 0,
                median_read: f64::NAN,
            },
        );
        assert_eq!(
            serde_json::to_value(&row).unwrap(),
            serde_json::json!({
                "dimensions": { "utm_source": "linkedin" },
                "metrics": { "visitors": 3, "visits": 4, "bounce_rate": 0.25, "visit_duration": 50.0, "median_read_s": null },
                "goals": { "signup": 1 },
                "conversion_rates": { "signup": 1.0 / 3.0 },
            })
        );
    }

    fn raw(visitors: u64, goals: Vec<u64>) -> RawRow {
        RawRow {
            dims: vec![],
            visitors,
            pageviews: visitors,
            goals,
            visits: 0,
            bounces: 0,
            duration_sum: 0,
            reads: 0,
            engaged_n: 0,
            median_read: 0.0,
        }
    }

    #[test]
    fn a_conversion_rate_is_converting_visitors_over_visitors() {
        let q = parse(with(
            base(),
            "goals",
            serde_json::json!(["signup", "thanks"]),
        ))
        .unwrap();
        let plan = Plan::of(&q);
        let row = shape(&q, &plan, raw(4, vec![1, 0]));
        assert_eq!(
            serde_json::to_value(&row.conversion_rates).unwrap(),
            serde_json::json!({ "signup": 0.25, "thanks": 0.0 })
        );
        // No visitors, no rate; never above 1.
        let row = shape(&q, &plan, raw(0, vec![1, 0]));
        assert_eq!(row.conversion_rates["signup"], Value::Null);
        let row = shape(&q, &plan, raw(1, vec![2, 1]));
        assert_eq!(row.conversion_rates["signup"], 1.0);
        // Without goals the field is left out of the answer.
        let q = parse(base()).unwrap();
        let row = shape(&q, &Plan::of(&q), raw(4, vec![]));
        assert!(
            serde_json::to_value(&row)
                .unwrap()
                .get("conversion_rates")
                .is_none()
        );
    }

    #[test]
    fn goals_count_the_rows_visitors_whatever_the_metrics() {
        let (sql, plan) = sql_of(serde_json::json!({
            "date_from": "2026-09-01", "date_to": "2026-09-30",
            "dimensions": ["utm_source"], "metrics": ["visits"], "goals": ["signup"],
        }));
        assert!(plan.pageviews && plan.events && plan.sessions);
        assert!(
            sql.text
                .contains("AND (event_name = 'pageview' OR (event_name = ?))"),
            "{}",
            sql.text
        );
    }

    #[test]
    fn several_sites_are_one_query_and_each_keeps_its_own_hostnames() {
        let q = parse(serde_json::json!({
            "date_from": "2026-09-01", "date_to": "2026-09-30",
            "dimensions": ["channel"], "metrics": ["visitors", "visits"],
        }))
        .unwrap();
        let b = SiteScope {
            ch_id: 9,
            hostnames: vec!["b.example".into()],
        };
        let (sql, plan) = build(&q, &[scope(), b]);
        assert!(plan.visit_join && plan.sessions);
        assert_eq!(sql.text.matches('?').count(), sql.binds.len());
        assert!(
            sql.text.contains("FROM sessions WHERE site_id IN (7, 9) AND date >= ? AND date <= ? GROUP BY session_id) AS v"),
            "{}",
            sql.text
        );
        assert!(
            sql.text.contains(
                "((site_id = 7 AND hostname IN (?, ?)) OR (site_id = 9 AND hostname IN (?)))"
            ),
            "{}",
            sql.text
        );
        // Each site is `self` for its own rows only.
        assert!(
            sql.text.contains("(site_id = 9 AND (0 OR h = ?"),
            "{}",
            sql.text
        );
        assert!(sql.binds.contains(&".b.example".to_string()));
    }

    #[test]
    fn the_channel_and_the_campaign_groups_are_v1_dimensions_of_a_visit() {
        for name in ["channel", "utm_source_group", "utm_medium_group"] {
            let d = Dimension::parse_v1(name).unwrap();
            assert!(d.is_session_level() && from_visit(d), "{name}");
        }
        let (sql, _) = sql_of(serde_json::json!({
            "date_from": "2026-09-01", "date_to": "2026-09-30",
            "dimensions": ["channel"], "metrics": ["visitors"],
            "filters": [{ "dimension": "utm_source_group", "operator": "is", "value": "other" }],
        }));
        // The channel reads the visit's referrer, or the event's own.
        assert!(
            sql.text
                .contains("extractGroups(if(v_session_id != '', v_referrer, referrer), "),
            "{}",
            sql.text
        );
        assert!(sql.binds.last().is_some_and(|b| b == "other"));
    }
}
