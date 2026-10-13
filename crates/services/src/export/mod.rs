use clickhouse::Client;
use serde::Deserialize;
use thiserror::Error;

use crate::analytics::scope::{SiteScope, events_scope};
use crate::analytics::sql::Sql;

#[derive(Debug, Error)]
pub enum ExportError {
    #[error("ClickHouse error: {0}")]
    ClickHouse(#[from] clickhouse::error::Error),
    #[error("Invalid date: {0}")]
    InvalidDate(String),
}

pub struct ExportService {
    client: Client,
}

const HEADER: &str = "time,event_name,path,referrer,referrer_source,country,browser,os,device_type,utm_source,utm_medium,utm_campaign\n";

impl ExportService {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    /// Every event of the site's hostnames in the date range, oldest first,
    /// as CSV.
    pub async fn export_csv(
        &self,
        scope: &SiteScope,
        date_from: &str,
        date_to: &str,
    ) -> Result<String, ExportError> {
        // The dates come from the query string: parsed strictly, then bound,
        // never spliced into the SQL (GHSA-7f5h-5qwr-rxh5).
        let parse = |d: &str| {
            crate::analytics::query::parse_date(d)
                .map(|d| d.to_string())
                .map_err(|_| ExportError::InvalidDate(d.to_string()))
        };
        let (date_from, date_to) = (parse(date_from)?, parse(date_to)?);

        // The rows are read as RowBinary, which the client asks for itself,
        // and written out as CSV here.
        let mut sql = Sql::new(
            "SELECT \
                formatDateTime(timestamp, '%Y-%m-%d %H:%M:%S') AS time, \
                event_name, path, referrer, referrer_source, \
                country, browser, os, device_type, \
                utm_source, utm_medium, utm_campaign \
             FROM events WHERE ",
        );
        sql.append(&events_scope(std::slice::from_ref(scope)));
        sql.push(" AND date >= ").bind(date_from);
        sql.push(" AND date <= ").bind(date_to);
        sql.push(" ORDER BY timestamp");

        let rows = sql.query(&self.client).fetch_all::<CsvRow>().await?;

        let mut csv = String::from(HEADER);
        for row in rows {
            let fields = [
                &row.time,
                &row.event_name,
                &row.path,
                &row.referrer,
                &row.referrer_source,
                &row.country,
                &row.browser,
                &row.os,
                &row.device_type,
                &row.utm_source,
                &row.utm_medium,
                &row.utm_campaign,
            ];
            let line: Vec<String> = fields.iter().map(|f| csv_field(f)).collect();
            csv.push_str(&line.join(","));
            csv.push('\n');
        }

        Ok(csv)
    }
}

/// One CSV field: quoted, with its quotes doubled, when it holds a comma, a
/// quote or a line break (RFC 4180).
fn csv_field(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

#[derive(Debug, clickhouse::Row, Deserialize)]
struct CsvRow {
    time: String,
    event_name: String,
    path: String,
    referrer: String,
    referrer_source: String,
    country: String,
    browser: String,
    os: String,
    device_type: String,
    utm_source: String,
    utm_medium: String,
    utm_campaign: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_field_with_a_comma_or_a_quote_is_quoted() {
        assert_eq!(csv_field("/blog/a"), "/blog/a");
        assert_eq!(csv_field("a,b"), "\"a,b\"");
        assert_eq!(csv_field("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(csv_field("two\nlines"), "\"two\nlines\"");
    }
}
