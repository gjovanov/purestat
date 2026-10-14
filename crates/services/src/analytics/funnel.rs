//! Funnels (PS4): how many visitors went through a site's ordered steps
//! within one visit, step by step, with the conversion between the steps,
//! overall and by the visit's source or campaign. `docs/funnels.md` is the
//! feature, `docs/api.md` the contract.
//!
//! **The visit is the unit.** The events of a visit (one `session_id`) are
//! read in time order by ClickHouse's `windowFunnel` in its `'strict_once'`
//! mode:
//! - a step counts only once the step before it was reached, in the same
//!   visit;
//! - other events in between do not matter: another page, an engagement
//!   report, the pageview of the page a goal fires on;
//! - one event serves one step at most, so `/blog/*` followed by
//!   `/blog/pricing` takes two pageviews;
//! - the window is every day the query reads, so it never cuts a visit: the
//!   visit is the only boundary.
//!
//! The other modes are wrong for this, and a test pins each: `'strict_order'`
//! fails a visit as soon as any other event comes between two steps, which
//! the engagement report between two pageviews always does; the default mode
//! lets one event complete two adjacent steps; `'strict_increase'` loses a
//! step when one event matches two adjacent steps after a repeat.
//! `'strict_once'` needs ClickHouse 24.10 or newer.
//!
//! A visit belongs to the day it started, as the visit metrics of the v1
//! query do. Its source and campaign are those of its first event, which is
//! what its row in `sessions` holds, and which a visit still in progress
//! (without that row) has too. A visitor counts once per step, however many
//! of their visits reached it.

use std::collections::HashSet;

use bson::oid::ObjectId;
use purestat_db::models::{Funnel, FunnelStep, FunnelStepType, Goal, GoalType};
use serde::{Deserialize, Serialize};

use super::query::{QueryError, QueryService, parse_date};
use super::scope::{SiteScope, events_scope};
use super::sql::{DateRange, Dimension, Filter, Operator, Sql, Table};
use super::v1::{GoalCondition, GoalSpec, MAX_SPAN_DAYS, V1Filter, push_goal_condition};

pub const MAX_NAME_CHARS: usize = 100;
pub const MAX_LABEL_CHARS: usize = 100;
pub const MAX_PATTERN_CHARS: usize = 512;
pub const DEFAULT_BREAKDOWN_LIMIT: u64 = 10;
pub const MAX_BREAKDOWN_LIMIT: u64 = 100;

/// The `windowFunnel` mode; see the module comment.
const MODE: &str = "'strict_once'";

/// The visit attributes a report can be broken down and filtered by: where
/// the visit came from (its source, referrer host and channel), and its
/// campaign (the `utm_*` parameters and the taxonomy's groups).
pub const VISIT_DIMENSIONS: [Dimension; 10] = [
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
];

/// A funnel as a create or an update sends it.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FunnelBody {
    pub name: String,
    pub steps: Vec<StepBody>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StepBody {
    #[serde(rename = "type")]
    pub step_type: FunnelStepType,
    pub pattern: Option<String>,
    pub goal_id: Option<String>,
    pub label: Option<String>,
}

/// A definition that passed every check: what is stored.
#[derive(Debug, Clone, PartialEq)]
pub struct FunnelDefinition {
    pub name: String,
    pub steps: Vec<FunnelStep>,
}

impl FunnelDefinition {
    /// Checks a create or update against the rules and the site's goals. The
    /// error is the message a 422 carries.
    pub fn check(body: FunnelBody, site_goals: &[Goal]) -> Result<Self, String> {
        let name = body.name.trim();
        if name.is_empty() {
            return Err("name is required".to_string());
        }
        if name.chars().count() > MAX_NAME_CHARS {
            return Err(format!("name is longer than {MAX_NAME_CHARS} characters"));
        }
        let count = body.steps.len();
        if !(Funnel::MIN_STEPS..=Funnel::MAX_STEPS).contains(&count) {
            return Err(format!(
                "a funnel has {} to {} steps, not {count}",
                Funnel::MIN_STEPS,
                Funnel::MAX_STEPS
            ));
        }
        let steps = body
            .steps
            .into_iter()
            .enumerate()
            .map(|(i, step)| check_step(i + 1, step, site_goals))
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Self {
            name: name.to_string(),
            steps,
        })
    }
}

/// The text, trimmed, when there is any.
fn given(value: &Option<String>) -> Option<&str> {
    value.as_deref().map(str::trim).filter(|v| !v.is_empty())
}

fn check_step(n: usize, step: StepBody, site_goals: &[Goal]) -> Result<FunnelStep, String> {
    let label = given(&step.label).map(str::to_string);
    if label
        .as_ref()
        .is_some_and(|l| l.chars().count() > MAX_LABEL_CHARS)
    {
        return Err(format!(
            "step {n}: the label is longer than {MAX_LABEL_CHARS} characters"
        ));
    }
    match step.step_type {
        FunnelStepType::Page => {
            if given(&step.goal_id).is_some() {
                return Err(format!(
                    "step {n}: a page step has a pattern, not a goal_id"
                ));
            }
            let pattern = given(&step.pattern)
                .ok_or_else(|| format!("step {n}: a page step needs a pattern"))?;
            if !(pattern.starts_with('/') || pattern.starts_with('*')) {
                return Err(format!(
                    "step {n}: a page pattern starts with / or *, not {pattern:?}"
                ));
            }
            if pattern.chars().count() > MAX_PATTERN_CHARS {
                return Err(format!(
                    "step {n}: the pattern is longer than {MAX_PATTERN_CHARS} characters"
                ));
            }
            Ok(FunnelStep {
                step_type: FunnelStepType::Page,
                pattern: Some(pattern.to_string()),
                goal_id: None,
                label,
            })
        }
        FunnelStepType::Goal => {
            if given(&step.pattern).is_some() {
                return Err(format!(
                    "step {n}: a goal step has a goal_id, not a pattern"
                ));
            }
            let goal_id = given(&step.goal_id)
                .and_then(|id| ObjectId::parse_str(id).ok())
                .filter(|id| site_goals.iter().any(|g| g.id == Some(*id)))
                .ok_or_else(|| format!("step {n}: goal_id is not a goal of this site"))?;
            Ok(FunnelStep {
                step_type: FunnelStepType::Goal,
                pattern: None,
                goal_id: Some(goal_id),
                label,
            })
        }
    }
}

/// A page pattern as a `LIKE` pattern: `*` is any run of characters, `/`
/// included, and every other character, `%`, `_` and `\` too, is itself.
pub fn like_pattern(pattern: &str) -> String {
    let mut like = String::with_capacity(pattern.len() + 4);
    for c in pattern.chars() {
        match c {
            '*' => like.push('%'),
            '%' | '_' | '\\' => {
                like.push('\\');
                like.push(c);
            }
            c => like.push(c),
        }
    }
    like
}

/// What a step matches, ready for SQL.
#[derive(Debug, Clone, PartialEq)]
pub enum StepCondition {
    /// A pageview of exactly this path.
    Path(String),
    /// A pageview whose path matches this `LIKE` pattern.
    PathLike(String),
    /// A completion of a goal, matched as the v1 query counts goals.
    Goal(GoalSpec),
    /// A goal step whose goal was deleted: nothing completes it.
    Nothing,
}

impl StepCondition {
    pub fn of(step: &FunnelStep, site_goals: &[Goal]) -> Self {
        match step.step_type {
            FunnelStepType::Page => {
                let pattern = step.pattern.clone().unwrap_or_default();
                if pattern.contains('*') {
                    Self::PathLike(like_pattern(&pattern))
                } else {
                    Self::Path(pattern)
                }
            }
            FunnelStepType::Goal => match find_goal(step, site_goals) {
                Some(goal) => Self::Goal(goal_spec(goal)),
                None => Self::Nothing,
            },
        }
    }
}

fn find_goal<'a>(step: &FunnelStep, site_goals: &'a [Goal]) -> Option<&'a Goal> {
    step.goal_id
        .and_then(|id| site_goals.iter().find(|g| g.id == Some(id)))
}

/// One goal, as the v1 query reads a goal.
fn goal_spec(goal: &Goal) -> GoalSpec {
    let condition = match goal.goal_type {
        GoalType::CustomEvent => goal.event_name.clone().map(GoalCondition::Event),
        GoalType::Pageview => goal.page_path.clone().map(GoalCondition::Page),
    };
    GoalSpec {
        name: goal.name.clone(),
        conditions: condition.into_iter().collect(),
    }
}

/// What a step is called: its label, else its pattern, else its goal's name.
pub fn step_label(step: &FunnelStep, site_goals: &[Goal]) -> String {
    if let Some(label) = &step.label {
        return label.clone();
    }
    match step.step_type {
        FunnelStepType::Page => step.pattern.clone().unwrap_or_default(),
        FunnelStepType::Goal => find_goal(step, site_goals)
            .map(|g| g.name.clone())
            .unwrap_or_else(|| "Deleted goal".to_string()),
    }
}

/// Whether a goal step's goal no longer exists.
pub fn goal_missing(step: &FunnelStep, site_goals: &[Goal]) -> bool {
    step.step_type == FunnelStepType::Goal && find_goal(step, site_goals).is_none()
}

/// A report request, as sent.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FunnelReportRequest {
    pub date_from: String,
    pub date_to: String,
    pub breakdown: Option<String>,
    pub filters: Option<Vec<V1Filter>>,
    pub limit: Option<u64>,
}

/// A report request that passed every check.
#[derive(Debug, Clone)]
pub struct FunnelQuery {
    pub dates: DateRange,
    pub breakdown: Option<Dimension>,
    pub filters: Vec<Filter>,
    pub limit: u64,
}

/// A visit attribute by its v1 name; anything else is refused.
fn visit_dimension(what: &str, name: &str) -> Result<Dimension, String> {
    Dimension::parse_v1(name)
        .filter(|d| VISIT_DIMENSIONS.contains(d))
        .ok_or_else(|| {
            let names: Vec<&str> = VISIT_DIMENSIONS.iter().map(|d| d.name()).collect();
            format!("unknown {what}: {name:?} (one of {})", names.join(", "))
        })
}

impl FunnelQuery {
    /// Checks a report request. The error is the message a 400 carries.
    pub fn parse(req: FunnelReportRequest) -> Result<Self, String> {
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
        let breakdown = req
            .breakdown
            .as_deref()
            .map(|name| visit_dimension("breakdown", name))
            .transpose()?;
        let filters = req
            .filters
            .unwrap_or_default()
            .into_iter()
            .map(|f| {
                Ok(Filter {
                    dimension: visit_dimension("filter dimension", &f.dimension)?,
                    operator: Operator::parse(&f.operator)
                        .ok_or_else(|| format!("unknown filter operator: {:?}", f.operator))?,
                    value: f.value,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let limit = req.limit.unwrap_or(DEFAULT_BREAKDOWN_LIMIT);
        if !(1..=MAX_BREAKDOWN_LIMIT).contains(&limit) {
            return Err(format!("limit must be between 1 and {MAX_BREAKDOWN_LIMIT}"));
        }
        Ok(Self {
            dates: DateRange::new(from, to),
            breakdown,
            filters,
            limit,
        })
    }
}

/// The column a visit attribute takes in the per-visit rows.
fn visit_column(d: Dimension) -> String {
    format!("v_{}", d.name())
}

/// `windowFunnel`'s window, in milliseconds: every day the query reads (the
/// range, and a day either side), so the window never cuts a visit.
fn window_ms(dates: &DateRange) -> u64 {
    let days = (dates.to - dates.from).num_days().max(0) as u64 + 3;
    days * 86_400_000
}

fn push_condition(sql: &mut Sql, condition: &StepCondition) {
    match condition {
        StepCondition::Path(path) => {
            sql.push("(event_name = 'pageview' AND path = ")
                .bind(path.clone());
            sql.push(")");
        }
        StepCondition::PathLike(like) => {
            sql.push("(event_name = 'pageview' AND path LIKE ")
                .bind(like.clone());
            sql.push(")");
        }
        StepCondition::Goal(goal) => push_goal_condition(sql, goal),
        StepCondition::Nothing => {
            sql.push("0");
        }
    }
}

/// One row per visit that started in the range, on the site's hostnames:
/// its visitor, the attributes the query reads, and how many steps it
/// reached in order (`level`).
fn visits_sql(
    conditions: &[StepCondition],
    scope: &SiteScope,
    q: &FunnelQuery,
    breakdown: Option<Dimension>,
) -> Sql {
    let mut sql = Sql::new("SELECT session_id, any(visitor_hash) AS visitor");
    let mut seen = HashSet::new();
    for d in breakdown
        .into_iter()
        .chain(q.filters.iter().map(|f| f.dimension))
    {
        if !seen.insert(d) {
            continue;
        }
        // As every report reads the dimension: a campaign source normalised,
        // a channel from the referrer's host and the site's own hostnames.
        let expr = d
            .expr(Table::Events, std::slice::from_ref(scope))
            .expect("a visit attribute is on every event");
        sql.push(", argMin(")
            .append(&expr)
            .push(&format!(", timestamp) AS {}", visit_column(d)));
    }
    sql.push(&format!(
        ", windowFunnel({}, {MODE})(toUInt64(toUnixTimestamp64Milli(timestamp))",
        window_ms(&q.dates)
    ));
    for condition in conditions {
        sql.push(", ");
        push_condition(&mut sql, condition);
    }
    sql.push(") AS level FROM events WHERE ");
    sql.append(&events_scope(std::slice::from_ref(scope)));
    // A visit that started on the range's last day can run past midnight,
    // and one that started the day before can run into the first: read a
    // day either side, and keep the visits by the day they started.
    sql.push(" AND date >= ").bind(q.dates.visits_from());
    sql.push(" AND date <= ").bind(q.dates.events_to());
    sql.push(" GROUP BY session_id HAVING toDate(min(timestamp)) >= ")
        .bind(q.dates.from_str());
    sql.push(" AND toDate(min(timestamp)) <= ")
        .bind(q.dates.to_str());
    sql
}

/// The distinct visitors who reached each step: one row of totals, or one
/// row per value of `breakdown`, the most visitors at the first step first.
fn report_sql(
    conditions: &[StepCondition],
    scope: &SiteScope,
    q: &FunnelQuery,
    breakdown: Option<Dimension>,
) -> Sql {
    let mut sql = Sql::new("SELECT ");
    match breakdown {
        Some(d) => sql.push(&format!("{} AS value", visit_column(d))),
        None => sql.push("'' AS value"),
    };
    let reached: Vec<String> = (1..=conditions.len())
        .map(|k| format!("uniqExactIf(visitor, level >= {k})"))
        .collect();
    sql.push(&format!(", [{}] AS visitors FROM (", reached.join(", ")));
    sql.append(&visits_sql(conditions, scope, q, breakdown));
    sql.push(")");
    for (i, filter) in q.filters.iter().enumerate() {
        sql.push(if i == 0 { " WHERE " } else { " AND " });
        filter.predicate(&visit_column(filter.dimension), &mut sql);
    }
    if breakdown.is_some() {
        // One row past the page says whether there are more values.
        sql.push(&format!(
            " GROUP BY value ORDER BY visitors[1] DESC, value ASC LIMIT {}",
            q.limit + 1
        ));
    }
    sql
}

/// A step's numbers.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct StepNumbers {
    /// Distinct visitors who reached the step.
    pub visitors: u64,
    /// The share of the previous step's visitors who reached this one, 0 to
    /// 1; null for the first step, or when the previous step has none.
    pub conversion_from_previous: Option<f64>,
    /// The share of the first step's visitors who reached this one, 0 to 1;
    /// null when the first step has none.
    pub conversion_from_start: Option<f64>,
    /// The previous step's visitors who did not reach this one; null for
    /// the first step.
    pub dropoff: Option<u64>,
}

/// The numbers of every step, from the visitors who reached each.
pub fn step_numbers(visitors: &[u64]) -> Vec<StepNumbers> {
    let share = |n: u64, of: u64| (of > 0).then(|| n as f64 / of as f64);
    let first = visitors.first().copied().unwrap_or(0);
    visitors
        .iter()
        .enumerate()
        .map(|(i, &n)| {
            let previous = i.checked_sub(1).map(|p| visitors[p]);
            StepNumbers {
                visitors: n,
                conversion_from_previous: previous.and_then(|p| share(n, p)),
                conversion_from_start: share(n, first),
                dropoff: previous.map(|p| p.saturating_sub(n)),
            }
        })
        .collect()
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct BreakdownRow {
    pub value: String,
    pub steps: Vec<StepNumbers>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunnelReport {
    pub steps: Vec<StepNumbers>,
    /// One row per value of the breakdown, when one was asked for.
    pub breakdown: Option<Vec<BreakdownRow>>,
    pub has_more: bool,
}

#[derive(Debug, clickhouse::Row, Deserialize)]
struct RawRow {
    value: String,
    visitors: Vec<u64>,
}

impl QueryService {
    /// The funnel's numbers over the site's scope: one query for the totals
    /// and, with a breakdown, one for its rows.
    pub async fn query_funnel(
        &self,
        scope: &SiteScope,
        conditions: &[StepCondition],
        q: &FunnelQuery,
    ) -> Result<FunnelReport, QueryError> {
        let totals = report_sql(conditions, scope, q, None)
            .query(self.client())
            .fetch_all::<RawRow>();
        let rows = async {
            match q.breakdown {
                Some(d) => report_sql(conditions, scope, q, Some(d))
                    .query(self.client())
                    .fetch_all::<RawRow>()
                    .await
                    .map(Some),
                None => Ok(None),
            }
        };
        let (totals, rows) = tokio::try_join!(totals, rows)?;

        let mut reached = totals
            .into_iter()
            .next()
            .map(|r| r.visitors)
            .unwrap_or_default();
        reached.resize(conditions.len(), 0);
        let has_more = rows.as_ref().is_some_and(|r| r.len() as u64 > q.limit);
        let breakdown = rows.map(|rows| {
            rows.into_iter()
                .take(q.limit as usize)
                .map(|r| BreakdownRow {
                    value: r.value,
                    steps: step_numbers(&r.visitors),
                })
                .collect()
        });
        Ok(FunnelReport {
            steps: step_numbers(&reached),
            breakdown,
            has_more,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bson::DateTime;

    fn goal(
        id: u8,
        name: &str,
        goal_type: GoalType,
        event: Option<&str>,
        page: Option<&str>,
    ) -> Goal {
        Goal {
            id: Some(ObjectId::from_bytes([id; 12])),
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
            goal(1, "Signup", GoalType::CustomEvent, Some("signup"), None),
            goal(2, "Thanks", GoalType::Pageview, None, Some("/thanks")),
        ]
    }

    fn body(value: serde_json::Value) -> FunnelBody {
        serde_json::from_value(value).expect("a funnel body")
    }

    fn check(value: serde_json::Value) -> Result<FunnelDefinition, String> {
        FunnelDefinition::check(body(value), &goals())
    }

    fn page(pattern: &str) -> serde_json::Value {
        serde_json::json!({ "type": "page", "pattern": pattern })
    }

    #[test]
    fn a_definition_keeps_its_steps_in_order_trimmed() {
        let signup = goals()[0].id.unwrap();
        let def = check(serde_json::json!({
            "name": "  Blog to signup ",
            "steps": [
                { "type": "page", "pattern": " /blog/* ", "label": " Read a post " },
                { "type": "page", "pattern": "/pricing", "label": "  " },
                { "type": "goal", "goal_id": signup.to_hex() },
            ],
        }))
        .unwrap();
        assert_eq!(def.name, "Blog to signup");
        assert_eq!(
            def.steps,
            [
                FunnelStep {
                    step_type: FunnelStepType::Page,
                    pattern: Some("/blog/*".into()),
                    goal_id: None,
                    label: Some("Read a post".into()),
                },
                FunnelStep {
                    step_type: FunnelStepType::Page,
                    pattern: Some("/pricing".into()),
                    goal_id: None,
                    label: None,
                },
                FunnelStep {
                    step_type: FunnelStepType::Goal,
                    pattern: None,
                    goal_id: Some(signup),
                    label: None,
                },
            ]
        );
    }

    #[test]
    fn a_definition_breaking_a_rule_is_refused_with_the_reason() {
        let other_sites_goal = ObjectId::new().to_hex();
        for (value, reason) in [
            (
                serde_json::json!({ "name": " ", "steps": [page("/a"), page("/b")] }),
                "name is required",
            ),
            (
                serde_json::json!({ "name": "x".repeat(101), "steps": [page("/a"), page("/b")] }),
                "name is longer",
            ),
            (
                serde_json::json!({ "name": "One", "steps": [page("/a")] }),
                "2 to 8 steps, not 1",
            ),
            (
                serde_json::json!({ "name": "Nine", "steps": vec![page("/a"); 9] }),
                "2 to 8 steps, not 9",
            ),
            (
                serde_json::json!({ "name": "F", "steps": [page("/a"), { "type": "page" }] }),
                "step 2: a page step needs a pattern",
            ),
            (
                serde_json::json!({ "name": "F", "steps": [page("pricing"), page("/b")] }),
                "step 1: a page pattern starts with / or *",
            ),
            (
                serde_json::json!({ "name": "F", "steps": [page("/a"), page(&format!("/{}", "x".repeat(512)))] }),
                "step 2: the pattern is longer",
            ),
            (
                serde_json::json!({ "name": "F", "steps": [page("/a"), { "type": "goal", "goal_id": other_sites_goal }] }),
                "step 2: goal_id is not a goal of this site",
            ),
            (
                serde_json::json!({ "name": "F", "steps": [page("/a"), { "type": "goal", "goal_id": "not-an-id" }] }),
                "step 2: goal_id is not a goal of this site",
            ),
            (
                serde_json::json!({ "name": "F", "steps": [page("/a"), { "type": "goal" }] }),
                "step 2: goal_id is not a goal of this site",
            ),
            (
                serde_json::json!({ "name": "F", "steps": [{ "type": "page", "pattern": "/a", "goal_id": "x" }, page("/b")] }),
                "step 1: a page step has a pattern, not a goal_id",
            ),
            (
                serde_json::json!({ "name": "F", "steps": [page("/a"), page("/b"), { "type": "page", "pattern": "/c", "label": "x".repeat(101) }] }),
                "step 3: the label is longer",
            ),
        ] {
            let err = check(value.clone()).unwrap_err();
            assert!(err.contains(reason), "{value} -> {err}");
        }
        let goal_step_with_a_pattern = serde_json::json!({
            "name": "F",
            "steps": [page("/a"), { "type": "goal", "goal_id": goals()[0].id.unwrap().to_hex(), "pattern": "/x" }],
        });
        assert!(
            check(goal_step_with_a_pattern)
                .unwrap_err()
                .contains("a goal step has a goal_id, not a pattern")
        );
        assert!(
            serde_json::from_value::<FunnelBody>(serde_json::json!({
                "name": "F", "steps": [page("/a"), page("/b")], "site_id": "x"
            }))
            .is_err(),
            "unknown fields are refused"
        );
        assert!(
            serde_json::from_value::<FunnelBody>(serde_json::json!({
                "name": "F", "steps": [page("/a"), { "type": "event", "pattern": "/b" }]
            }))
            .is_err(),
            "unknown step types are refused"
        );
    }

    #[test]
    fn a_star_is_any_run_of_characters_and_everything_else_is_literal() {
        assert_eq!(like_pattern("/blog/*"), "/blog/%");
        assert_eq!(like_pattern("*/pricing"), "%/pricing");
        assert_eq!(like_pattern("/docs/*/install"), "/docs/%/install");
        assert_eq!(like_pattern("/sale/50%_off*"), "/sale/50\\%\\_off%");
        assert_eq!(like_pattern("/a\\b"), "/a\\\\b");
        let steps: Vec<FunnelStep> = check(serde_json::json!({
            "name": "F", "steps": [page("/blog/*"), page("/50%_off")],
        }))
        .unwrap()
        .steps;
        assert_eq!(
            StepCondition::of(&steps[0], &[]),
            StepCondition::PathLike("/blog/%".into())
        );
        assert_eq!(
            StepCondition::of(&steps[1], &[]),
            StepCondition::Path("/50%_off".into()),
            "without a star, a pattern is an exact path"
        );
    }

    #[test]
    fn a_goal_step_matches_its_goal_and_a_deleted_goal_matches_nothing() {
        let site_goals = goals();
        let step = |goal_id: ObjectId| FunnelStep {
            step_type: FunnelStepType::Goal,
            pattern: None,
            goal_id: Some(goal_id),
            label: None,
        };
        let signup = step(site_goals[0].id.unwrap());
        let thanks = step(site_goals[1].id.unwrap());
        let deleted = step(ObjectId::new());
        assert_eq!(
            StepCondition::of(&signup, &site_goals),
            StepCondition::Goal(GoalSpec {
                name: "Signup".into(),
                conditions: vec![GoalCondition::Event("signup".into())],
            })
        );
        assert_eq!(
            StepCondition::of(&thanks, &site_goals),
            StepCondition::Goal(GoalSpec {
                name: "Thanks".into(),
                conditions: vec![GoalCondition::Page("/thanks".into())],
            })
        );
        assert_eq!(
            StepCondition::of(&deleted, &site_goals),
            StepCondition::Nothing
        );
        assert_eq!(step_label(&signup, &site_goals), "Signup");
        assert_eq!(step_label(&deleted, &site_goals), "Deleted goal");
        assert!(goal_missing(&deleted, &site_goals));
        assert!(!goal_missing(&signup, &site_goals));
    }

    fn report(value: serde_json::Value) -> Result<FunnelQuery, String> {
        FunnelQuery::parse(serde_json::from_value(value).expect("a report request"))
    }

    #[test]
    fn a_report_request_is_checked() {
        let q = report(serde_json::json!({ "date_from": "2026-09-01", "date_to": "2026-09-30" }))
            .unwrap();
        assert_eq!(q.limit, DEFAULT_BREAKDOWN_LIMIT);
        assert!(q.breakdown.is_none() && q.filters.is_empty());
        let q = report(serde_json::json!({
            "date_from": "2026-09-01", "date_to": "2026-09-30", "breakdown": "utm_campaign",
            "filters": [{ "dimension": "referrer_source", "operator": "is_not", "value": "Direct" }],
            "limit": 100,
        }))
        .unwrap();
        assert_eq!(q.breakdown, Some(Dimension::UtmCampaign));
        assert_eq!(q.filters[0].dimension, Dimension::ReferrerSource);

        for (value, reason) in [
            (
                serde_json::json!({ "date_from": "2026-09-30", "date_to": "2026-09-01" }),
                "before",
            ),
            (
                serde_json::json!({ "date_from": "2025-01-01", "date_to": "2026-02-05" }),
                "366 days",
            ),
            (
                serde_json::json!({ "date_from": "2026-09-01' OR 1=1", "date_to": "2026-09-30" }),
                "date_from",
            ),
            (
                serde_json::json!({ "date_from": "2026-09-01", "date_to": "2026-09-30", "breakdown": "path" }),
                "unknown breakdown",
            ),
            (
                serde_json::json!({ "date_from": "2026-09-01", "date_to": "2026-09-30", "breakdown": "source" }),
                "unknown breakdown",
            ),
            (
                serde_json::json!({ "date_from": "2026-09-01", "date_to": "2026-09-30",
                    "filters": [{ "dimension": "country", "operator": "is", "value": "DE" }] }),
                "unknown filter dimension",
            ),
            (
                serde_json::json!({ "date_from": "2026-09-01", "date_to": "2026-09-30",
                    "filters": [{ "dimension": "utm_source", "operator": "like", "value": "x" }] }),
                "unknown filter operator",
            ),
            (
                serde_json::json!({ "date_from": "2026-09-01", "date_to": "2026-09-30", "limit": 0 }),
                "limit",
            ),
            (
                serde_json::json!({ "date_from": "2026-09-01", "date_to": "2026-09-30", "limit": 101 }),
                "limit",
            ),
        ] {
            let err = report(value.clone()).unwrap_err();
            assert!(err.contains(reason), "{value} -> {err}");
        }
        assert!(
            serde_json::from_value::<FunnelReportRequest>(serde_json::json!({
                "date_from": "2026-09-01", "date_to": "2026-09-30", "period": "30d"
            }))
            .is_err(),
            "unknown fields are refused"
        );
    }

    fn scope() -> SiteScope {
        SiteScope {
            ch_id: 7,
            hostnames: vec!["a.example".into(), "www.a.example".into()],
        }
    }

    #[test]
    fn every_value_is_bound_and_the_funnel_runs_per_visit_in_strict_once_mode() {
        let hostile = "x' OR 1=1 OR '";
        let site_goals = goals();
        let conditions = vec![
            StepCondition::PathLike(like_pattern(&format!("/{hostile}*"))),
            StepCondition::Path(hostile.into()),
            StepCondition::of(
                &FunnelStep {
                    step_type: FunnelStepType::Goal,
                    pattern: None,
                    goal_id: site_goals[0].id,
                    label: None,
                },
                &site_goals,
            ),
            StepCondition::Nothing,
        ];
        let q = report(serde_json::json!({
            "date_from": "2026-09-01", "date_to": "2026-09-30", "breakdown": "utm_source",
            "filters": [
                { "dimension": "utm_campaign", "operator": "is", "value": hostile },
                { "dimension": "utm_source", "operator": "starts_with", "value": hostile },
            ],
        }))
        .unwrap();
        let sql = report_sql(&conditions, &scope(), &q, q.breakdown);
        assert!(!sql.text.contains(hostile), "{}", sql.text);
        assert_eq!(
            sql.text.matches('?').count(),
            sql.binds.len(),
            "{}",
            sql.text
        );
        assert_eq!(sql.binds.iter().filter(|b| b.contains(hostile)).count(), 4);

        // 32 days read (the 30 of the range and a day either side), as
        // milliseconds.
        assert!(
            sql.text.contains(&format!(
                "windowFunnel({}, 'strict_once')(toUInt64(toUnixTimestamp64Milli(timestamp)), \
                 (event_name = 'pageview' AND path LIKE ?), (event_name = 'pageview' AND path = ?), \
                 (event_name = ?), 0) AS level",
                32 * 86_400_000u64
            )),
            "{}",
            sql.text
        );
        assert!(
            sql.text.contains("GROUP BY session_id HAVING toDate(min(timestamp)) >= ? AND toDate(min(timestamp)) <= ?"),
            "a visit counts on the day it started: {}",
            sql.text
        );
        assert!(
            sql.text.contains("FROM events WHERE ((site_id = 7 AND hostname IN (?, ?))) AND date >= ? AND date <= ?"),
            "the site's hostnames only: {}",
            sql.text
        );
        // The source as every report reads it: normalised.
        let source = Dimension::UtmSource
            .expr(Table::Events, &[scope()])
            .unwrap()
            .text;
        assert!(
            sql.text.contains(&format!(
                ", argMin({source}, timestamp) AS v_utm_source, argMin(utm_campaign, timestamp) AS v_utm_campaign,"
            )),
            "the breakdown and the filters read the visit's first event, once each: {}",
            sql.text
        );
        assert_eq!(sql.text.matches("AS v_utm_source").count(), 1);

        // A channel binds the site's own hostnames, as in the dashboard.
        let by_channel = report(serde_json::json!({
            "date_from": "2026-09-01", "date_to": "2026-09-30", "breakdown": "channel",
        }))
        .unwrap();
        let channel = report_sql(&conditions, &scope(), &by_channel, by_channel.breakdown);
        assert!(channel.text.contains("AS v_channel"), "{}", channel.text);
        assert_eq!(
            channel.text.matches('?').count(),
            channel.binds.len(),
            "{}",
            channel.text
        );
        assert!(
            channel.binds.iter().any(|b| b == ".a.example"),
            "{:?}",
            channel.binds
        );
        assert!(
            sql.text.contains(") WHERE v_utm_campaign = ? AND startsWith(v_utm_source, ?) GROUP BY value ORDER BY visitors[1] DESC, value ASC LIMIT 11"),
            "{}",
            sql.text
        );
        assert!(sql.text.starts_with(
            "SELECT v_utm_source AS value, [uniqExactIf(visitor, level >= 1), uniqExactIf(visitor, level >= 2), \
             uniqExactIf(visitor, level >= 3), uniqExactIf(visitor, level >= 4)] AS visitors FROM ("
        ), "{}", sql.text);
        let days: Vec<&str> = sql
            .binds
            .iter()
            .filter(|b| b.starts_with("2026-"))
            .map(String::as_str)
            .collect();
        assert_eq!(
            days,
            ["2026-08-31", "2026-10-01", "2026-09-01", "2026-09-30"]
        );

        let totals = report_sql(&conditions, &scope(), &q, None);
        assert!(
            totals.text.starts_with("SELECT '' AS value, ["),
            "{}",
            totals.text
        );
        assert!(!totals.text.contains("GROUP BY value"), "{}", totals.text);
        assert!(
            !totals.text.contains("v_utm_source AS value"),
            "{}",
            totals.text
        );
    }

    #[test]
    fn conversion_is_a_share_of_the_step_before_and_of_the_first() {
        assert_eq!(
            step_numbers(&[4, 2, 1]),
            [
                StepNumbers {
                    visitors: 4,
                    conversion_from_previous: None,
                    conversion_from_start: Some(1.0),
                    dropoff: None,
                },
                StepNumbers {
                    visitors: 2,
                    conversion_from_previous: Some(0.5),
                    conversion_from_start: Some(0.5),
                    dropoff: Some(2),
                },
                StepNumbers {
                    visitors: 1,
                    conversion_from_previous: Some(0.5),
                    conversion_from_start: Some(0.25),
                    dropoff: Some(1),
                },
            ]
        );
        let none = step_numbers(&[0, 0]);
        assert_eq!(none[0].conversion_from_start, None);
        assert_eq!(none[1].conversion_from_previous, None);
        assert_eq!(none[1].dropoff, Some(0));
    }
}
