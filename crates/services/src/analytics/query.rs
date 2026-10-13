use clickhouse::Client;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::scope::SiteScope;
use super::sql::{DateRange, Dimension, Filter, Operator, Sql, events_where, sessions_where};

#[derive(Debug, Error)]
pub enum QueryError {
    #[error("ClickHouse error: {0}")]
    ClickHouse(#[from] clickhouse::error::Error),
    #[error("Invalid query: {0}")]
    InvalidQuery(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsQuery {
    pub date_range: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub metrics: Vec<String>,
    pub dimensions: Option<Vec<String>>,
    pub filters: Option<Vec<StatsFilter>>,
    pub interval: Option<String>,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsFilter {
    pub dimension: String,
    pub operator: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsResult {
    pub metrics: serde_json::Value,
    pub dimensions: Option<Vec<DimensionResult>>,
    pub timeseries: Option<Vec<TimeseriesPoint>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DimensionResult {
    pub dimension: String,
    pub value: String,
    pub metrics: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeseriesPoint {
    pub date: String,
    pub metrics: serde_json::Value,
}

/// A calendar date as the API accepts one: `YYYY-MM-DD` and nothing else.
/// Anything that is not a date is refused before it can reach a query.
pub fn parse_date(s: &str) -> Result<chrono::NaiveDate, QueryError> {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|_| QueryError::InvalidQuery(format!("Invalid date: {s}")))
}

/// The dashboard's queries. Every one reads a site through its scope (its
/// ClickHouse id and the hostnames that count for it), and every visitor and
/// pageview count is over `pageview` events.
pub struct QueryService {
    client: Client,
}

impl QueryService {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    pub async fn query_stats(
        &self,
        scope: &SiteScope,
        query: &StatsQuery,
    ) -> Result<StatsResult, QueryError> {
        self.query_stats_sites(std::slice::from_ref(scope), query)
            .await
    }

    /// The same query across several sites of one org. The route resolves
    /// every site within the caller's org before it gets here.
    pub async fn query_stats_multi(
        &self,
        scopes: &[SiteScope],
        query: &StatsQuery,
    ) -> Result<StatsResult, QueryError> {
        if scopes.is_empty() {
            return Err(QueryError::InvalidQuery("No site IDs provided".to_string()));
        }
        self.query_stats_sites(scopes, query).await
    }

    async fn query_stats_sites(
        &self,
        scopes: &[SiteScope],
        query: &StatsQuery,
    ) -> Result<StatsResult, QueryError> {
        let dates = Self::resolve_date_range(query)?;
        let filters = Self::parse_filters(query)?;

        if let Some(dimensions) = &query.dimensions {
            // Dimension breakdown: visitors and pageviews per value.
            let name = dimensions.first().ok_or_else(|| {
                QueryError::InvalidQuery("dimensions must name at least one dimension".to_string())
            })?;
            let dimension = Dimension::parse(name)
                .ok_or_else(|| QueryError::InvalidQuery(format!("Unknown dimension: {name}")))?;
            let limit = query.limit.unwrap_or(10);
            let offset = query.offset.unwrap_or(0);

            let sql = Self::breakdown_sql(scopes, &dates, &filters, dimension, limit, offset);
            let rows = sql.query(&self.client).fetch_all::<DimensionRow>().await?;

            let dimension_results: Vec<DimensionResult> = rows
                .into_iter()
                .map(|r| DimensionResult {
                    dimension: name.clone(),
                    value: r.dimension,
                    metrics: serde_json::json!({
                        "visitors": r.visitors,
                        "pageviews": r.pageviews,
                    }),
                })
                .collect();

            Ok(StatsResult {
                metrics: serde_json::json!({}),
                dimensions: Some(dimension_results),
                timeseries: None,
            })
        } else if query.interval.is_some() {
            // Timeseries
            let interval = query.interval.as_deref().unwrap_or("day");
            let date_trunc = match interval {
                "minute" => "toString(toStartOfMinute(timestamp))",
                "hour" => "toString(toStartOfHour(timestamp))",
                "day" => "toString(toDate(timestamp))",
                "week" => "toString(toMonday(timestamp))",
                "month" => "toString(toStartOfMonth(timestamp))",
                _ => "toString(toDate(timestamp))",
            };

            let mut sql = Sql::new(format!(
                "SELECT {date_trunc} AS period, uniq(visitor_hash) AS visitors, count() AS pageviews FROM events WHERE "
            ));
            sql.append(&events_where(scopes, &dates, &filters, true));
            sql.push(" GROUP BY period ORDER BY period");

            let rows = sql.query(&self.client).fetch_all::<TimeseriesRow>().await?;

            let timeseries: Vec<TimeseriesPoint> = rows
                .into_iter()
                .map(|r| TimeseriesPoint {
                    date: r.period,
                    metrics: serde_json::json!({
                        "visitors": r.visitors,
                        "pageviews": r.pageviews,
                    }),
                })
                .collect();

            Ok(StatsResult {
                metrics: serde_json::json!({}),
                dimensions: None,
                timeseries: Some(timeseries),
            })
        } else {
            // Aggregate metrics: visitors and pageviews from events, bounce
            // rate and visit duration from sessions.
            let mut sql = Sql::new(
                "SELECT uniq(visitor_hash) AS visitors, count() AS pageviews FROM events WHERE ",
            );
            sql.append(&events_where(scopes, &dates, &filters, true));
            let row = sql
                .query(&self.client)
                .fetch_one::<BaseAggregateRow>()
                .await?;

            let mut session_sql = Sql::new(
                "SELECT ifNotFinite(round(countIf(is_bounce = 1) / count() * 100, 1), 0) AS bounce_rate, \
                 ifNotFinite(round(avg(duration), 0), 0) AS visit_duration FROM sessions WHERE ",
            );
            session_sql.append(&sessions_where(scopes, &dates, &filters));
            let session_row = session_sql
                .query(&self.client)
                .fetch_one::<SessionAggregateRow>()
                .await?;

            Ok(StatsResult {
                metrics: serde_json::json!({
                    "visitors": row.visitors,
                    "pageviews": row.pageviews,
                    "bounce_rate": session_row.bounce_rate,
                    "visit_duration": session_row.visit_duration,
                }),
                dimensions: None,
                timeseries: None,
            })
        }
    }

    /// Visitors and pageviews per value of one dimension. A page or a visit
    /// attribute that every event carries is read from `events`; the entry
    /// and exit page only a visit has are read from `sessions`, where a
    /// value's pageviews are those of the visits that entered or left there.
    fn breakdown_sql(
        scopes: &[SiteScope],
        dates: &DateRange,
        filters: &[Filter],
        dimension: Dimension,
        limit: u64,
        offset: u64,
    ) -> Sql {
        let mut sql;
        if let Some(expr) = dimension.events_expr() {
            sql = Sql::new(format!(
                "SELECT {expr} AS dimension, uniq(visitor_hash) AS visitors, count() AS pageviews FROM events WHERE "
            ));
            sql.append(&events_where(scopes, dates, filters, true));
        } else {
            let expr = dimension
                .sessions_expr()
                .expect("a dimension lives on at least one table");
            sql = Sql::new(format!(
                "SELECT {expr} AS dimension, uniq(visitor_hash) AS visitors, sum(pageviews) AS pageview_total FROM sessions WHERE "
            ));
            sql.append(&sessions_where(scopes, dates, filters));
        }
        sql.push(&format!(
            " GROUP BY dimension ORDER BY visitors DESC, dimension ASC LIMIT {limit} OFFSET {offset}"
        ));
        sql
    }

    /// The filters, each on a known dimension with a known operator. Values
    /// stay values: they are bound, never spliced (GHSA-7f5h-5qwr-rxh5).
    fn parse_filters(query: &StatsQuery) -> Result<Vec<Filter>, QueryError> {
        query
            .filters
            .iter()
            .flatten()
            .map(|f| {
                let dimension = Dimension::parse(&f.dimension).ok_or_else(|| {
                    QueryError::InvalidQuery(format!("Unknown dimension: {}", f.dimension))
                })?;
                let operator = Operator::parse(&f.operator).ok_or_else(|| {
                    QueryError::InvalidQuery(format!("Unknown operator: {}", f.operator))
                })?;
                Ok(Filter {
                    dimension,
                    operator,
                    value: f.value.clone(),
                })
            })
            .collect()
    }

    fn resolve_date_range(query: &StatsQuery) -> Result<DateRange, QueryError> {
        if let (Some(from), Some(to)) = (&query.date_from, &query.date_to) {
            return Ok(DateRange::new(parse_date(from)?, parse_date(to)?));
        }

        let range = query.date_range.as_deref().unwrap_or("30d");
        let today = chrono::Utc::now().date_naive();
        let from = match range {
            "day" | "1d" => today,
            "7d" => today - chrono::Duration::days(7),
            "30d" => today - chrono::Duration::days(30),
            "6mo" => today - chrono::Duration::days(180),
            "12mo" => today - chrono::Duration::days(365),
            _ => today - chrono::Duration::days(30),
        };

        Ok(DateRange::new(from, today))
    }
}

#[derive(Debug, clickhouse::Row, Deserialize)]
struct DimensionRow {
    dimension: String,
    visitors: u64,
    pageviews: u64,
}

#[derive(Debug, clickhouse::Row, Deserialize)]
struct TimeseriesRow {
    period: String,
    visitors: u64,
    pageviews: u64,
}

#[derive(Debug, clickhouse::Row, Deserialize)]
struct BaseAggregateRow {
    visitors: u64,
    pageviews: u64,
}

#[derive(Debug, clickhouse::Row, Deserialize)]
struct SessionAggregateRow {
    bounce_rate: f64,
    visit_duration: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(filters: Vec<(&str, &str, &str)>) -> StatsQuery {
        StatsQuery {
            date_range: None,
            date_from: Some("2026-09-01".into()),
            date_to: Some("2026-09-30".into()),
            metrics: vec![],
            dimensions: None,
            filters: Some(
                filters
                    .into_iter()
                    .map(|(d, o, v)| StatsFilter {
                        dimension: d.into(),
                        operator: o.into(),
                        value: v.into(),
                    })
                    .collect(),
            ),
            interval: None,
            limit: None,
            offset: None,
        }
    }

    fn scope() -> SiteScope {
        SiteScope {
            ch_id: 7,
            hostnames: vec!["a.example".into()],
        }
    }

    #[test]
    fn dates_must_be_dates() {
        let mut q = query(vec![]);
        q.date_from = Some("2026-09-01' OR '1'='1".into());
        assert!(matches!(
            QueryService::resolve_date_range(&q),
            Err(QueryError::InvalidQuery(_))
        ));
        assert_eq!(parse_date("2026-09-01").unwrap().to_string(), "2026-09-01");
        assert!(parse_date("2026-09-01 ").is_err());
        assert!(parse_date("yesterday").is_err());
    }

    #[test]
    fn unknown_columns_and_operators_are_refused() {
        assert!(QueryService::parse_filters(&query(vec![("password", "is", "x")])).is_err());
        assert!(QueryService::parse_filters(&query(vec![("path", "matches", "x")])).is_err());
        assert_eq!(
            QueryService::parse_filters(&query(vec![("page", "contains", "x")]))
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn a_page_breakdown_counts_pageviews_of_the_sites_hostnames() {
        let q = query(vec![]);
        let dates = QueryService::resolve_date_range(&q).unwrap();
        let sql = QueryService::breakdown_sql(&[scope()], &dates, &[], Dimension::Path, 10, 0);
        assert!(sql.text.starts_with("SELECT path AS dimension, uniq(visitor_hash) AS visitors, count() AS pageviews FROM events WHERE ((site_id = 7 AND hostname IN (?))) AND date >= ? AND date <= ? AND event_name = 'pageview'"), "{}", sql.text);
        assert_eq!(sql.binds, ["a.example", "2026-09-01", "2026-09-30"]);
    }

    #[test]
    fn an_entry_page_breakdown_reads_sessions() {
        let q = query(vec![]);
        let dates = QueryService::resolve_date_range(&q).unwrap();
        let sql = QueryService::breakdown_sql(&[scope()], &dates, &[], Dimension::EntryPage, 10, 0);
        assert!(sql.text.starts_with("SELECT entry_page AS dimension, uniq(visitor_hash) AS visitors, sum(pageviews) AS pageview_total FROM sessions WHERE site_id IN (7)"), "{}", sql.text);
        assert!(sql.text.contains("session_id IN (SELECT session_id FROM events WHERE ((site_id = 7 AND hostname IN (?)))"), "{}", sql.text);
        assert_eq!(sql.text.matches('?').count(), sql.binds.len());
    }
}
