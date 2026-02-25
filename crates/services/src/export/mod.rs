use clickhouse::Client;
use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExportError {
    #[error("ClickHouse error: {0}")]
    ClickHouse(#[from] clickhouse::error::Error),
}

pub struct ExportService {
    client: Client,
}

impl ExportService {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    pub async fn export_csv(
        &self,
        site_id: u64,
        date_from: &str,
        date_to: &str,
    ) -> Result<String, ExportError> {
        let sql = format!(
            "SELECT \
                formatDateTime(timestamp, '%Y-%m-%d %H:%M:%S') as time, \
                event_name, path, referrer, referrer_source, \
                country, browser, os, device_type, \
                utm_source, utm_medium, utm_campaign \
             FROM events \
             WHERE site_id = {site_id} AND date >= '{date_from}' AND date <= '{date_to}' \
             ORDER BY timestamp \
             FORMAT CSVWithNames"
        );

        let rows = self
            .client
            .query(&sql)
            .fetch_all::<CsvRow>()
            .await?;

        let mut csv = String::from(
            "time,event_name,path,referrer,referrer_source,country,browser,os,device_type,utm_source,utm_medium,utm_campaign\n",
        );
        for row in rows {
            csv.push_str(&format!(
                "{},{},{},{},{},{},{},{},{},{},{},{}\n",
                row.time,
                row.event_name,
                row.path,
                row.referrer,
                row.referrer_source,
                row.country,
                row.browser,
                row.os,
                row.device_type,
                row.utm_source,
                row.utm_medium,
                row.utm_campaign
            ));
        }

        Ok(csv)
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
