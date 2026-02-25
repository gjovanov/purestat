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
        let (date_from, date_to) = self.resolve_date_range(query)?;

        // Build base WHERE clause
        let mut conditions = vec![
            format!("site_id = {site_id}"),
            format!("date >= '{date_from}'"),
            format!("date <= '{date_to}'"),
        ];

        // Apply filters
        if let Some(filters) = &query.filters {
            for f in filters {
                let col = Self::dimension_to_column(&f.dimension)?;
                let condition = match f.operator.as_str() {
                    "is" => format!("{col} = '{}'", f.value),
                    "is_not" => format!("{col} != '{}'", f.value),
                    "contains" => format!("{col} LIKE '%{}%'", f.value),
                    _ => {
                        return Err(QueryError::InvalidQuery(format!(
                            "Unknown operator: {}",
                            f.operator
                        )))
                    }
                };
                conditions.push(condition);
            }
        }

        let where_clause = conditions.join(" AND ");

        // Build SELECT based on metrics
        let metrics_sql = self.build_metrics_sql(&query.metrics)?;

        if let Some(dimensions) = &query.dimensions {
            // Dimension breakdown
            let dim_col = Self::dimension_to_column(&dimensions[0])?;
            let limit = query.limit.unwrap_or(10);
            let offset = query.offset.unwrap_or(0);

            let sql = format!(
                "SELECT {dim_col} as dimension, {metrics_sql} FROM events WHERE {where_clause} GROUP BY dimension ORDER BY visitors DESC LIMIT {limit} OFFSET {offset}"
            );

            let rows = self
                .client
                .query(&sql)
                .fetch_all::<DimensionRow>()
                .await?;

            let dimension_results: Vec<DimensionResult> = rows
                .into_iter()
                .map(|r| DimensionResult {
                    dimension: dimensions[0].clone(),
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
                "minute" => "toStartOfMinute(timestamp)",
                "hour" => "toStartOfHour(timestamp)",
                "day" => "toDate(timestamp)",
                "week" => "toMonday(timestamp)",
                "month" => "toStartOfMonth(timestamp)",
                _ => "toDate(timestamp)",
            };

            let sql = format!(
                "SELECT {date_trunc} as period, {metrics_sql} FROM events WHERE {where_clause} GROUP BY period ORDER BY period"
            );

            let rows = self
                .client
                .query(&sql)
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
                .client
                .query(&sql)
                .fetch_one::<BaseAggregateRow>()
                .await?;

            // Bounce rate and visit duration require the sessions table
            let session_sql = format!(
                "SELECT round(countIf(is_bounce = 1) / count() * 100, 1) as bounce_rate, round(avg(duration), 0) as visit_duration FROM sessions WHERE {where_clause}"
            );
            let session_row = self
                .client
                .query(&session_sql)
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

    fn resolve_date_range(&self, query: &StatsQuery) -> Result<(String, String), QueryError> {
        if let (Some(from), Some(to)) = (&query.date_from, &query.date_to) {
            return Ok((from.clone(), to.clone()));
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

    fn build_metrics_sql(&self, metrics: &[String]) -> Result<String, QueryError> {
        let parts: Vec<String> = metrics
            .iter()
            .map(|m| match m.as_str() {
                "visitors" => Ok("uniq(visitor_hash) as visitors".to_string()),
                "pageviews" => Ok("count() as pageviews".to_string()),
                "bounce_rate" => Ok(
                    "round(sum(is_bounce) / uniq(session_id) * 100, 1) as bounce_rate"
                        .to_string(),
                ),
                "visit_duration" => {
                    Ok("round(avg(duration), 0) as visit_duration".to_string())
                }
                "events" => Ok("count() as events".to_string()),
                _ => Err(QueryError::InvalidQuery(format!(
                    "Unknown metric: {m}"
                ))),
            })
            .collect::<Result<Vec<_>, _>>()?;

        if parts.is_empty() {
            Ok("uniq(visitor_hash) as visitors, count() as pageviews".to_string())
        } else {
            Ok(parts.join(", "))
        }
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
