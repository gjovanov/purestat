//! Journeys (PS5): what visitors did next after landing on a page, and the
//! pages that led into a goal, as a tree of paths a Sankey draws.
//! `docs/journeys.md` is the feature, `docs/api.md` the contract.
//!
//! **A path is a visit's pages in order** (pageviews only, a reload of the
//! same page collapsed), cut to at most `depth` pages:
//! - *from an entry page*: the pages after it, in the visits that entered
//!   the site there, then an exit when the visit ended within the depth;
//! - *into a goal*: the pages before the visit's first completion of the
//!   goal, the nearest first, then an entrance when the visit started
//!   within the depth.
//!
//! Every prefix of a path is a node of the tree. **k-anonymity is a product
//! rule: a node is shown only when at least `MIN_VISITORS` distinct visitors
//! followed its path**, and the query itself drops every other one
//! (`HAVING visitors >= 5`), so a rarer path never leaves ClickHouse. Their
//! visits fold into one "other" node under their parent, computed from what
//! is shown: the parent's visits less those of its shown children. The root
//! obeys the rule too: with fewer than 5 visitors there is no tree at all.
//!
//! The tree is bounded so that no query can explode: at most `MAX_DEPTH`
//! levels, at most `ROWS_PER_LEVEL` rows per level from ClickHouse, and at
//! most `width` pages per level on screen (the rest fold into "other").
//!
//! Visits are counted as the funnels count them: on the site's own
//! hostnames, on the day they started (UTC), and visits, not visitors, are
//! the flows, so that a parent's flow is the sum of its children's.

use std::collections::HashMap;

use purestat_db::models::{Goal, GoalType};
use serde::{Deserialize, Serialize};

use super::query::{QueryError, QueryService, parse_date};
use super::scope::{SiteScope, events_scope};
use super::sql::{DateRange, Sql};
use super::v1::{GoalCondition, GoalSpec, MAX_SPAN_DAYS, push_goal_condition};

/// The fewest distinct visitors a path needs to be shown.
pub const MIN_VISITORS: u64 = 5;
pub const DEFAULT_DEPTH: u8 = 3;
pub const MAX_DEPTH: u8 = 5;
pub const DEFAULT_WIDTH: usize = 5;
pub const MAX_WIDTH: usize = 10;
/// The most paths per level a query returns, the most visited first: a
/// bound on what leaves ClickHouse, far above what a report shows.
pub const ROWS_PER_LEVEL: u64 = 100;
pub const MAX_PAGE_CHARS: usize = 512;

/// A report request, as sent: exactly one of `entry_page` and `goal_id`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JourneyRequest {
    pub date_from: String,
    pub date_to: String,
    pub entry_page: Option<String>,
    pub goal_id: Option<String>,
    pub depth: Option<u8>,
    pub width: Option<usize>,
}

/// Where the paths start from.
#[derive(Debug, Clone, PartialEq)]
pub enum Anchor {
    /// The visits that entered the site on this page, and what they did next.
    EntryPage(String),
    /// The visits that completed this goal, and what they did before.
    Goal(GoalSpec),
}

impl Anchor {
    fn direction(&self) -> Direction {
        match self {
            Self::EntryPage(_) => Direction::Next,
            Self::Goal(_) => Direction::Into,
        }
    }

    fn label(&self) -> String {
        match self {
            Self::EntryPage(page) => page.clone(),
            Self::Goal(goal) => goal.name.clone(),
        }
    }
}

/// One goal, as the v1 query reads a goal.
pub fn goal_spec(goal: &Goal) -> GoalSpec {
    let condition = match goal.goal_type {
        GoalType::CustomEvent => goal.event_name.clone().map(GoalCondition::Event),
        GoalType::Pageview => goal.page_path.clone().map(GoalCondition::Page),
    };
    GoalSpec {
        name: goal.name.clone(),
        conditions: condition.into_iter().collect(),
    }
}

/// A request that passed every check.
#[derive(Debug, Clone)]
pub struct JourneyQuery {
    pub dates: DateRange,
    pub anchor: Anchor,
    pub depth: u8,
    pub width: usize,
}

impl JourneyQuery {
    /// Checks a request; `site_goals` resolves its `goal_id`. The error is
    /// the message a 400 carries.
    pub fn parse(req: JourneyRequest, site_goals: &[Goal]) -> Result<Self, String> {
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
        let anchor = match (req.entry_page, req.goal_id) {
            (Some(page), None) => {
                let page = page.trim();
                if page.is_empty() || page.chars().count() > MAX_PAGE_CHARS {
                    return Err(format!(
                        "entry_page must be a path of 1 to {MAX_PAGE_CHARS} characters"
                    ));
                }
                Anchor::EntryPage(page.to_string())
            }
            (None, Some(id)) => {
                let goal = bson::oid::ObjectId::parse_str(id.trim())
                    .ok()
                    .and_then(|id| site_goals.iter().find(|g| g.id == Some(id)))
                    .ok_or_else(|| "goal_id is not a goal of this site".to_string())?;
                Anchor::Goal(goal_spec(goal))
            }
            _ => return Err("give exactly one of entry_page and goal_id".to_string()),
        };
        let depth = req.depth.unwrap_or(DEFAULT_DEPTH);
        if !(1..=MAX_DEPTH).contains(&depth) {
            return Err(format!("depth must be between 1 and {MAX_DEPTH}"));
        }
        let width = req.width.unwrap_or(DEFAULT_WIDTH);
        if !(1..=MAX_WIDTH).contains(&width) {
            return Err(format!("width must be between 1 and {MAX_WIDTH}"));
        }
        Ok(Self {
            dates: DateRange::new(from, to),
            anchor,
            depth,
            width,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    /// From the anchor forward: the pages after it.
    Next,
    /// Into the anchor: the pages before it, nearest first.
    Into,
}

/// One element of a path: a page, or the visit's end (an exit going
/// forward, an entrance going back).
pub type Step = (u8, String);
const PAGE: u8 = 0;
const END: u8 = 1;

/// A visit's pageviews, as `(milliseconds, path)` in time order.
const PAGEVIEWS: &str = "arraySort(x -> x.1, groupArrayIf((toUnixTimestamp64Milli(timestamp), path), \
     event_name = 'pageview'))";

/// One row per visit that started in the range, on the site's hostnames,
/// with the path the anchor gives it: `visitor` and `seq`.
fn paths_sql(scope: &SiteScope, q: &JourneyQuery) -> Sql {
    let d = q.depth;
    let mut visits = Sql::new(format!(
        "SELECT any(visitor_hash) AS visitor, {PAGEVIEWS} AS pvs"
    ));
    if let Anchor::Goal(goal) = &q.anchor {
        visits.push(", minIf(toUnixTimestamp64Milli(timestamp), ");
        push_goal_condition(&mut visits, goal);
        visits.push(") AS converted_at");
    }
    visits.push(" FROM events WHERE ");
    visits.append(&events_scope(std::slice::from_ref(scope)));
    // A visit that started on the range's last day can run past midnight,
    // and one that started the day before can run into the first: read a
    // day either side, and keep the visits by the day they started.
    visits.push(" AND date >= ").bind(q.dates.visits_from());
    visits.push(" AND date <= ").bind(q.dates.events_to());
    visits
        .push(" GROUP BY session_id HAVING toDate(min(timestamp)) >= ")
        .bind(q.dates.from_str());
    visits
        .push(" AND toDate(min(timestamp)) <= ")
        .bind(q.dates.to_str());

    let mut sql;
    match &q.anchor {
        Anchor::EntryPage(page) => {
            // The pages after the entry page, then the exit when the visit
            // ended within the depth.
            sql = Sql::new(format!(
                "SELECT visitor, arrayConcat(arrayMap(p -> ({PAGE}, p), arraySlice(pages, 2, {d})), \
                 if(length(pages) - 1 < {d}, [({END}, '')], [])) AS seq FROM (\
                 SELECT visitor, arrayCompact(arrayMap(x -> x.2, pvs)) AS pages FROM ("
            ));
            sql.append(&visits);
            sql.push(")) WHERE length(pages) > 0 AND pages[1] = ")
                .bind(page.clone());
        }
        Anchor::Goal(_) => {
            // The pages before the first completion, nearest first, then
            // the entrance when the visit started within the depth.
            visits.push(" AND converted_at > 0");
            sql = Sql::new(format!(
                "SELECT visitor, arrayConcat(arrayMap(p -> ({PAGE}, p), arraySlice(before, 1, {d})), \
                 if(length(before) < {d}, [({END}, '')], [])) AS seq FROM (\
                 SELECT visitor, arrayReverse(arrayCompact(arrayMap(x -> x.2, \
                 arrayFilter(x -> x.1 < converted_at, pvs)))) AS before FROM ("
            ));
            sql.append(&visits);
            sql.push("))");
        }
    }
    sql
}

/// Every path prefix (the root's is empty) seen by at least `MIN_VISITORS`
/// distinct visitors, with its visits and visitors, the most visited first
/// within a level, at most `ROWS_PER_LEVEL` per level. The rule is the
/// HAVING here: no rarer path is ever returned.
pub fn journey_sql(scope: &SiteScope, q: &JourneyQuery) -> Sql {
    let mut sql = Sql::new(
        "SELECT toUInt8(length(prefix)) AS level, prefix, count() AS visits, \
         uniqExact(visitor) AS visitors FROM (SELECT visitor, arraySlice(seq, 1, n) AS prefix FROM (",
    );
    sql.append(&paths_sql(scope, q));
    sql.push(&format!(
        ") ARRAY JOIN range(0, length(seq) + 1) AS n) GROUP BY prefix \
         HAVING visitors >= {MIN_VISITORS} ORDER BY level, visits DESC, prefix \
         LIMIT {ROWS_PER_LEVEL} BY level"
    ));
    sql
}

/// A path prefix as the query returns it.
#[derive(Debug, Clone, PartialEq, clickhouse::Row, Deserialize)]
pub struct PathRow {
    pub level: u8,
    pub prefix: Vec<Step>,
    pub visits: u64,
    pub visitors: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    /// The entry page or the goal the paths start from.
    Anchor,
    Page,
    /// Going forward, the visits that ended here.
    Exit,
    /// Going back, the visits that started here.
    Entrance,
    /// The rest of the parent's visits: pages seen by fewer than 5 visitors,
    /// and pages past the width.
    Other,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JourneyNode {
    pub id: usize,
    /// Steps from the anchor: 0 is the anchor.
    pub level: u8,
    pub kind: NodeKind,
    /// The page's path, or the anchor's page or goal; empty for the others.
    pub label: String,
    pub visits: u64,
    /// Distinct visitors; null for "other", which is a remainder.
    pub visitors: Option<u64>,
}

/// A flow of visits. Going forward it runs from a node to the next page;
/// into a goal, from a page to the next one, towards the goal.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct JourneyLink {
    pub source: usize,
    pub target: usize,
    pub visits: u64,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct Journey {
    pub nodes: Vec<JourneyNode>,
    pub links: Vec<JourneyLink>,
}

/// The tree from the k-anonymous paths: the anchor, then per level the
/// `width` most visited pages whose parent is shown, every exit or
/// entrance that is shown, and per parent an "other" node for the rest of
/// its visits.
pub fn build(rows: &[PathRow], q: &JourneyQuery) -> Journey {
    let direction = q.anchor.direction();
    let mut journey = Journey::default();
    let Some(root) = rows.iter().find(|r| r.prefix.is_empty()) else {
        return journey;
    };
    journey.nodes.push(JourneyNode {
        id: 0,
        level: 0,
        kind: NodeKind::Anchor,
        label: q.anchor.label(),
        visits: root.visits,
        visitors: Some(root.visitors),
    });
    let link = |journey: &mut Journey, parent: usize, child: usize, visits: u64| {
        let (source, target) = match direction {
            Direction::Next => (parent, child),
            Direction::Into => (child, parent),
        };
        journey.links.push(JourneyLink {
            source,
            target,
            visits,
        });
    };

    // The shown pages of the previous level, by their path: their node and
    // visits. Only a page has children; an exit, an entrance and "other"
    // end their branch.
    let mut parents: HashMap<&[Step], (usize, u64)> =
        HashMap::from([(&root.prefix[..], (0, root.visits))]);
    for level in 1..=q.depth {
        let mut candidates: Vec<&PathRow> = rows
            .iter()
            .filter(|r| r.level == level)
            .filter(|r| parents.contains_key(&r.prefix[..r.prefix.len() - 1]))
            .collect();
        candidates.sort_by(|a, b| {
            b.visits
                .cmp(&a.visits)
                .then_with(|| a.prefix.cmp(&b.prefix))
        });

        let mut shown_visits: HashMap<&[Step], u64> = HashMap::new();
        let mut next: HashMap<&[Step], (usize, u64)> = HashMap::new();
        let mut pages = 0;
        for row in candidates {
            let (kind, label) = match row.prefix.last() {
                Some((END, _)) if direction == Direction::Next => (NodeKind::Exit, String::new()),
                Some((END, _)) => (NodeKind::Entrance, String::new()),
                Some((_, path)) => {
                    if pages == q.width {
                        continue;
                    }
                    pages += 1;
                    (NodeKind::Page, path.clone())
                }
                None => continue,
            };
            let parent_path = &row.prefix[..row.prefix.len() - 1];
            let (parent, _) = parents[parent_path];
            let id = journey.nodes.len();
            journey.nodes.push(JourneyNode {
                id,
                level,
                kind,
                label,
                visits: row.visits,
                visitors: Some(row.visitors),
            });
            link(&mut journey, parent, id, row.visits);
            *shown_visits.entry(parent_path).or_default() += row.visits;
            if kind == NodeKind::Page {
                next.insert(&row.prefix[..], (id, row.visits));
            }
        }

        // What is not shown of each parent's visits goes to its "other".
        let mut by_id: Vec<(&[Step], (usize, u64))> = parents.into_iter().collect();
        by_id.sort_by_key(|(_, (id, _))| *id);
        for (path, (parent, visits)) in by_id {
            let rest = visits.saturating_sub(shown_visits.get(path).copied().unwrap_or(0));
            if rest > 0 {
                let id = journey.nodes.len();
                journey.nodes.push(JourneyNode {
                    id,
                    level,
                    kind: NodeKind::Other,
                    label: String::new(),
                    visits: rest,
                    visitors: None,
                });
                link(&mut journey, parent, id, rest);
            }
        }
        parents = next;
        if parents.is_empty() {
            break;
        }
    }
    journey
}

impl QueryService {
    /// The k-anonymous paths of a journey: what `journey_sql` returns.
    pub async fn journey_paths(
        &self,
        scope: &SiteScope,
        q: &JourneyQuery,
    ) -> Result<Vec<PathRow>, QueryError> {
        Ok(journey_sql(scope, q)
            .query(self.client())
            .fetch_all::<PathRow>()
            .await?)
    }

    /// The journey's tree over the site's scope.
    pub async fn query_journey(
        &self,
        scope: &SiteScope,
        q: &JourneyQuery,
    ) -> Result<Journey, QueryError> {
        let rows = self.journey_paths(scope, q).await?;
        Ok(build(&rows, q))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bson::{DateTime, oid::ObjectId};

    fn goal() -> Goal {
        Goal {
            id: Some(ObjectId::from_bytes([3; 12])),
            site_id: ObjectId::new(),
            org_id: ObjectId::new(),
            goal_type: GoalType::CustomEvent,
            name: "Signup".into(),
            event_name: Some("signup".into()),
            page_path: None,
            created_at: DateTime::now(),
        }
    }

    fn parse(value: serde_json::Value) -> Result<JourneyQuery, String> {
        JourneyQuery::parse(
            serde_json::from_value(value).expect("a journey request"),
            &[goal()],
        )
    }

    fn scope() -> SiteScope {
        SiteScope {
            ch_id: 7,
            hostnames: vec!["a.example".into(), "www.a.example".into()],
        }
    }

    fn base() -> serde_json::Value {
        serde_json::json!({ "date_from": "2026-09-01", "date_to": "2026-09-30" })
    }

    fn with(mut body: serde_json::Value, key: &str, value: serde_json::Value) -> serde_json::Value {
        body[key] = value;
        body
    }

    #[test]
    fn a_request_names_one_anchor_and_stays_in_bounds() {
        let q = parse(with(base(), "entry_page", " /blog/a ".into())).unwrap();
        assert_eq!(q.anchor, Anchor::EntryPage("/blog/a".into()));
        assert_eq!((q.depth, q.width), (DEFAULT_DEPTH, DEFAULT_WIDTH));
        let q = parse(with(base(), "goal_id", goal().id.unwrap().to_hex().into())).unwrap();
        assert_eq!(
            q.anchor,
            Anchor::Goal(GoalSpec {
                name: "Signup".into(),
                conditions: vec![GoalCondition::Event("signup".into())],
            })
        );

        let page = |b: serde_json::Value| with(b, "entry_page", "/".into());
        for (body, reason) in [
            (base(), "exactly one"),
            (
                with(page(base()), "goal_id", goal().id.unwrap().to_hex().into()),
                "exactly one",
            ),
            (with(base(), "entry_page", " ".into()), "entry_page"),
            (
                with(base(), "goal_id", ObjectId::new().to_hex().into()),
                "not a goal",
            ),
            (with(page(base()), "depth", 0.into()), "depth"),
            (with(page(base()), "depth", 6.into()), "depth"),
            (with(page(base()), "width", 0.into()), "width"),
            (with(page(base()), "width", 11.into()), "width"),
            (
                with(page(base()), "date_to", "2027-09-30".into()),
                "366 days",
            ),
            (with(page(base()), "date_to", "2026-08-01".into()), "before"),
        ] {
            let err = parse(body.clone()).unwrap_err();
            assert!(err.contains(reason), "{body} -> {err}");
        }
        assert!(
            serde_json::from_value::<JourneyRequest>(with(page(base()), "min_visitors", 1.into()))
                .is_err(),
            "unknown fields are refused, so the threshold cannot be asked down"
        );
    }

    #[test]
    fn the_threshold_is_five_and_is_in_the_query() {
        assert_eq!(MIN_VISITORS, 5);
        let q = parse(with(base(), "entry_page", "/".into())).unwrap();
        let sql = journey_sql(&scope(), &q);
        assert!(
            sql.text
                .contains("GROUP BY prefix HAVING visitors >= 5 ORDER BY level, visits DESC, prefix LIMIT 100 BY level"),
            "{}",
            sql.text
        );
        assert!(
            sql.text.contains("uniqExact(visitor) AS visitors"),
            "distinct visitors, not visits: {}",
            sql.text
        );
    }

    #[test]
    fn every_value_is_bound_and_only_the_sites_hostnames_are_read() {
        let hostile = "/x' OR 1=1 OR '";
        for body in [
            with(base(), "entry_page", hostile.into()),
            with(base(), "goal_id", goal().id.unwrap().to_hex().into()),
        ] {
            let q = parse(body).unwrap();
            let sql = journey_sql(&scope(), &q);
            assert!(!sql.text.contains(hostile), "{}", sql.text);
            assert_eq!(
                sql.text.matches('?').count(),
                sql.binds.len(),
                "{}",
                sql.text
            );
            assert!(
                sql.text.contains("FROM events WHERE ((site_id = 7 AND hostname IN (?, ?))) AND date >= ? AND date <= ? GROUP BY session_id HAVING toDate(min(timestamp)) >= ? AND toDate(min(timestamp)) <= ?"),
                "{}",
                sql.text
            );
        }
        let q = parse(with(base(), "goal_id", goal().id.unwrap().to_hex().into())).unwrap();
        let sql = journey_sql(&scope(), &q);
        assert!(
            sql.text.contains(
                "minIf(toUnixTimestamp64Milli(timestamp), (event_name = ?)) AS converted_at"
            ) && sql.text.contains("AND converted_at > 0")
                && sql
                    .text
                    .contains("arrayFilter(x -> x.1 < converted_at, pvs)"),
            "the pages before the first completion: {}",
            sql.text
        );
    }

    fn row(prefix: &[&str], visits: u64, visitors: u64) -> PathRow {
        PathRow {
            level: prefix.len() as u8,
            prefix: prefix
                .iter()
                .map(|p| {
                    if *p == "$" {
                        (END, String::new())
                    } else {
                        (PAGE, p.to_string())
                    }
                })
                .collect(),
            visits,
            visitors,
        }
    }

    fn query(width: usize, depth: u8) -> JourneyQuery {
        let mut q = parse(with(base(), "entry_page", "/start".into())).unwrap();
        q.width = width;
        q.depth = depth;
        q
    }

    /// Every node as the labels of its path from the anchor, with its
    /// visits, sorted. A node's parent is the other end of its link to the
    /// level below it.
    fn paths(journey: &Journey) -> Vec<(Vec<String>, u64)> {
        let mut parent: HashMap<usize, usize> = HashMap::new();
        for l in &journey.links {
            let (s, t) = (&journey.nodes[l.source], &journey.nodes[l.target]);
            let (p, c) = if s.level < t.level {
                (s.id, t.id)
            } else {
                (t.id, s.id)
            };
            parent.insert(c, p);
        }
        let name = |n: &JourneyNode| match n.kind {
            NodeKind::Anchor | NodeKind::Page => n.label.clone(),
            NodeKind::Exit => "(exit)".to_string(),
            NodeKind::Entrance => "(entrance)".to_string(),
            NodeKind::Other => "(other)".to_string(),
        };
        let mut out: Vec<(Vec<String>, u64)> = journey
            .nodes
            .iter()
            .map(|n| {
                let mut path = vec![name(n)];
                let mut at = n.id;
                while let Some(&p) = parent.get(&at) {
                    path.push(name(&journey.nodes[p]));
                    at = p;
                }
                path.reverse();
                (path, n.visits)
            })
            .collect();
        out.sort();
        out
    }

    #[test]
    fn the_tree_keeps_the_widest_pages_and_folds_the_rest_of_each_parent_into_other() {
        // What the query would return: 25 visits entered on /start; /c and
        // /d were too rare to come back at all.
        let rows = vec![
            row(&[], 25, 22),
            row(&["/a"], 11, 11),
            row(&["$"], 5, 5),
            row(&["/a", "/b"], 6, 6),
            row(&["/a", "/e"], 5, 5),
            row(&["/a", "/b", "$"], 6, 6),
            row(&["/a", "/e", "/f"], 5, 5),
        ];
        let tree = build(&rows, &query(5, 3));
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert_eq!(
            paths(&tree),
            vec![
                (s(&["/start"]), 25),
                (s(&["/start", "(exit)"]), 5),
                (s(&["/start", "(other)"]), 9),
                (s(&["/start", "/a"]), 11),
                (s(&["/start", "/a", "/b"]), 6),
                (s(&["/start", "/a", "/b", "(exit)"]), 6),
                (s(&["/start", "/a", "/e"]), 5),
                (s(&["/start", "/a", "/e", "/f"]), 5),
            ]
        );
        // A flow leaves every node that is not a leaf with all its visits.
        for node in &tree.nodes {
            let out: u64 = tree
                .links
                .iter()
                .filter(|l| l.source == node.id)
                .map(|l| l.visits)
                .sum();
            assert!(out == 0 || out == node.visits, "{node:?}");
        }
        assert!(
            tree.nodes
                .iter()
                .all(|n| n.kind == NodeKind::Other || n.visitors.unwrap() >= MIN_VISITORS)
        );

        // One page per level: /e goes to "other" under /a, with its branch.
        let narrow = build(&rows, &query(1, 3));
        assert!(paths(&narrow).contains(&(s(&["/start", "/a", "(other)"]), 5)));
        assert!(
            !paths(&narrow)
                .iter()
                .any(|(p, _)| p.contains(&"/e".to_string()))
        );

        // Fewer than 5 visitors at the anchor: nothing at all.
        assert_eq!(build(&[], &query(5, 3)), Journey::default());
        assert_eq!(
            build(&rows, &query(5, 1))
                .nodes
                .iter()
                .map(|n| n.level)
                .max(),
            Some(1)
        );
    }

    #[test]
    fn into_a_goal_the_flows_run_towards_it() {
        let mut q = parse(with(base(), "goal_id", goal().id.unwrap().to_hex().into())).unwrap();
        q.depth = 2;
        let rows = vec![
            row(&[], 11, 11),
            row(&["/register"], 6, 6),
            row(&["/register", "/pricing"], 6, 6),
            row(&["/blog"], 5, 5),
            row(&["/blog", "$"], 5, 5),
        ];
        let tree = build(&rows, &q);
        assert_eq!(tree.nodes[0].label, "Signup");
        assert_eq!(tree.nodes[0].kind, NodeKind::Anchor);
        assert!(tree.links.iter().all(|l| l.target != l.source));
        let into_root: u64 = tree
            .links
            .iter()
            .filter(|l| l.target == 0)
            .map(|l| l.visits)
            .sum();
        assert_eq!(into_root, 11, "every flow ends at the goal");
        assert!(
            tree.nodes
                .iter()
                .any(|n| n.kind == NodeKind::Entrance && n.visits == 5)
        );
    }
}
