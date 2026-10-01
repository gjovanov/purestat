use clickhouse::Client;
use serde::{Deserialize, Serialize};
use thiserror::Error;

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

pub struct QueryService {
    client: Client,
}

impl QueryService {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    pub async fn query_stats(
        &self,
        site_id: u64,
        query: &StatsQuery,
    ) -> Result<StatsResult, QueryError> {
        self.query_stats_sites(&[site_id], query).await
    }

    /// The same query across several sites of one org. The route resolves
    /// every id within the caller's org before it gets here.
    pub async fn query_stats_multi(
        &self,
        site_ids: &[u64],
        query: &StatsQuery,
    ) -> Result<StatsResult, QueryError> {
        if site_ids.is_empty() {
            return Err(QueryError::InvalidQuery("No site IDs provided".to_string()));
        }
        self.query_stats_sites(site_ids, query).await
    }

    async fn query_stats_sites(
        &self,
        site_ids: &[u64],
        query: &StatsQuery,
    ) -> Result<StatsResult, QueryError> {
        let (where_clause, binds) = Self::build_where(site_ids, query)?;

        if let Some(dimensions) = &query.dimensions {
            // Dimension breakdown — always include visitors and pageviews
            let dimension = dimensions.first().ok_or_else(|| {
                QueryError::InvalidQuery("dimensions must name at least one dimension".to_string())
            })?;
            let dim_col = Self::dimension_to_column(dimension)?;
            let limit = query.limit.unwrap_or(10);
            let offset = query.offset.unwrap_or(0);

            let sql = format!(
                "SELECT {dim_col} as dimension, uniq(visitor_hash) as visitors, count() as pageviews FROM events WHERE {where_clause} GROUP BY dimension ORDER BY visitors DESC LIMIT {limit} OFFSET {offset}"
            );

            let rows = self
                .bound(&sql, &binds)
                .fetch_all::<DimensionRow>()
                .await?;

            let dimension_results: Vec<DimensionResult> = rows
                .into_iter()
                .map(|r| DimensionResult {
                    dimension: dimension.clone(),
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

            let sql = format!(
                "SELECT {date_trunc} as period, uniq(visitor_hash) as visitors, count() as pageviews FROM events WHERE {where_clause} GROUP BY period ORDER BY period"
            );

            let rows = self
                .bound(&sql, &binds)
                .fetch_all::<TimeseriesRow>()
                .await?;

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
            // Aggregate metrics only — always query the standard set from events table
            let sql = format!(
                "SELECT uniq(visitor_hash) as visitors, count() as pageviews FROM events WHERE {where_clause}"
            );

            let row = self
                .bound(&sql, &binds)
                .fetch_one::<BaseAggregateRow>()
                .await?;

            // Bounce rate and visit duration require the sessions table
            let session_sql = format!(
                "SELECT ifNaN(round(countIf(is_bounce = 1) / count() * 100, 1), 0) as bounce_rate, ifNaN(round(avg(duration), 0), 0) as visit_duration FROM sessions WHERE {where_clause}"
            );
            let session_row = self
                .bound(&session_sql, &binds)
                .fetch_one::<SessionAggregateRow>()
                .await
                .unwrap_or(SessionAggregateRow {
                    bounce_rate: 0.0,
                    visit_duration: 0.0,
                });

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

    /// The WHERE clause, and the values its `?` placeholders take, in order.
    ///
    /// Every caller-supplied VALUE is bound, never spliced into the SQL: the
    /// columns come from the `dimension_to_column` allowlist, the dates are
    /// parsed strictly, and filter values travel as bind parameters, so nothing
    /// a request sends can change the query (GHSA-7f5h-5qwr-rxh5).
    fn build_where(site_ids: &[u64], query: &StatsQuery) -> Result<(String, Vec<String>), QueryError> {
        let (date_from, date_to) = Self::resolve_date_range(query)?;
        // Site ids are u64s the route derived from ObjectIds it resolved within
        // the caller's org, never caller text, so they are safe as literals.
        let site_predicate = match site_ids {
            [one] => format!("site_id = {one}"),
            many => format!(
                "site_id IN ({})",
                many.iter().map(u64::to_string).collect::<Vec<_>>().join(", ")
            ),
        };
        let mut conditions = vec![
            site_predicate,
            "date >= ?".to_string(),
            "date <= ?".to_string(),
        ];
        let mut binds = vec![date_from, date_to];

        if let Some(filters) = &query.filters {
            for f in filters {
                let col = Self::dimension_to_column(&f.dimension)?;
                let condition = match f.operator.as_str() {
                    "is" => format!("{col} = ?"),
                    "is_not" => format!("{col} != ?"),
                    // position(), not LIKE: a value's `%` and `_` stay literal.
                    "contains" => format!("position({col}, ?) > 0"),
                    _ => {
                        return Err(QueryError::InvalidQuery(format!(
                            "Unknown operator: {}",
                            f.operator
                        )))
                    }
                };
                conditions.push(condition);
                binds.push(f.value.clone());
            }
        }

        Ok((conditions.join(" AND "), binds))
    }

    /// A query with the WHERE clause's values bound, in order.
    fn bound(&self, sql: &str, binds: &[String]) -> clickhouse::query::Query {
        binds.iter().fold(self.client.query(sql), |q, b| q.bind(b.as_str()))
    }

    fn resolve_date_range(query: &StatsQuery) -> Result<(String, String), QueryError> {
        if let (Some(from), Some(to)) = (&query.date_from, &query.date_to) {
            return Ok((parse_date(from)?.to_string(), parse_date(to)?.to_string()));
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

        Ok((from.to_string(), today.to_string()))
    }

    fn dimension_to_column(dimension: &str) -> Result<&str, QueryError> {
        match dimension {
            "source" | "referrer_source" => Ok("referrer_source"),
            "referrer" => Ok("referrer"),
            "page" | "path" => Ok("path"),
            "entry_page" => Ok("entry_page"),
            "exit_page" => Ok("exit_page"),
            "country" => Ok("country"),
            "region" => Ok("region"),
            "city" => Ok("city"),
            "browser" => Ok("browser"),
            "os" => Ok("os"),
            "device_type" => Ok("device_type"),
            "utm_source" => Ok("utm_source"),
            "utm_medium" => Ok("utm_medium"),
            "utm_campaign" => Ok("utm_campaign"),
            "utm_content" => Ok("utm_content"),
            "utm_term" => Ok("utm_term"),
            "event_name" => Ok("event_name"),
            _ => Err(QueryError::InvalidQuery(format!(
                "Unknown dimension: {dimension}"
            ))),
        }
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
                    .map(|(d, o, v)| StatsFilter { dimension: d.into(), operator: o.into(), value: v.into() })
                    .collect(),
            ),
            interval: None,
            limit: None,
            offset: None,
        }
    }

    #[test]
    fn several_sites_are_one_in_list_and_values_stay_bound() {
        let hostile = "x' OR '1'='1";
        let (sql, binds) = QueryService::build_where(&[7, 9], &query(vec![("path", "is", hostile)])).unwrap();
        assert!(sql.starts_with("site_id IN (7, 9) AND "), "{sql}");
        assert!(!sql.contains(hostile), "{sql}");
        assert_eq!(binds.last().map(String::as_str), Some(hostile));
    }

    #[test]
    fn a_filter_value_is_bound_never_spliced() {
        let hostile = "x' OR site_id != 0 OR '1'='1";
        let (sql, binds) = QueryService::build_where(&[7], &query(vec![("path", "is", hostile)])).unwrap();
        assert!(!sql.contains(hostile), "the value reached the SQL: {sql}");
        assert!(!sql.contains('\''), "no quote in the SQL at all: {sql}");
        assert_eq!(sql.matches('?').count(), binds.len());
        assert_eq!(binds, vec!["2026-09-01", "2026-09-30", hostile]);
    }

    #[test]
    fn contains_is_a_literal_substring_match() {
        let (sql, binds) = QueryService::build_where(&[7], &query(vec![("page", "contains", "50%_off")])).unwrap();
        assert!(sql.contains("position(path, ?) > 0"), "{sql}");
        assert!(!sql.contains("LIKE"), "{sql}");
        assert_eq!(binds.last().unwrap(), "50%_off");
    }

    #[test]
    fn dates_must_be_dates() {
        let mut q = query(vec![]);
        q.date_from = Some("2026-09-01' OR '1'='1".into());
        assert!(matches!(QueryService::build_where(&[7], &q), Err(QueryError::InvalidQuery(_))));
        assert_eq!(parse_date("2026-09-01").unwrap().to_string(), "2026-09-01");
        assert!(parse_date("2026-09-01 ").is_err());
        assert!(parse_date("yesterday").is_err());
    }

    #[test]
    fn unknown_columns_and_operators_are_refused() {
        assert!(QueryService::build_where(&[7], &query(vec![("password", "is", "x")])).is_err());
        assert!(QueryService::build_where(&[7], &query(vec![("path", "matches", "x")])).is_err());
    }
}
