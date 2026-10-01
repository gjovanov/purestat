use clickhouse::Client;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RealtimeError {
    #[error("ClickHouse error: {0}")]
    ClickHouse(#[from] clickhouse::error::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealtimeStats {
    pub current_visitors: u64,
    pub top_pages: Vec<RealtimePage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealtimePage {
    pub path: String,
    pub visitors: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealtimeVisitor {
    pub visitor_hash: String,
    pub country: String,
    pub site_id: u64,
    pub hostname: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiSiteRealtimeStats {
    pub current_visitors: u64,
    pub visitors: Vec<RealtimeVisitor>,
    pub top_pages: Vec<RealtimePage>,
}

pub struct RealtimeService {
    client: Client,
}

impl RealtimeService {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    pub async fn get_current_visitors(
        &self,
        site_id: u64,
    ) -> Result<RealtimeStats, RealtimeError> {
        // Visitors in last 5 minutes
        let count_sql = format!(
            "SELECT uniq(visitor_hash) as visitors FROM events \
             WHERE site_id = {site_id} AND timestamp >= now() - INTERVAL 5 MINUTE"
        );

        let row = self
            .client
            .query(&count_sql)
            .fetch_one::<VisitorCount>()
            .await?;

        // Top pages in last 5 minutes
        let pages_sql = format!(
            "SELECT path, uniq(visitor_hash) as visitors FROM events \
             WHERE site_id = {site_id} AND timestamp >= now() - INTERVAL 5 MINUTE \
             GROUP BY path ORDER BY visitors DESC LIMIT 10"
        );

        let pages = self
            .client
            .query(&pages_sql)
            .fetch_all::<PageRow>()
            .await?;

        Ok(RealtimeStats {
            current_visitors: row.visitors,
            top_pages: pages
                .into_iter()
                .map(|p| RealtimePage {
                    path: p.path,
                    visitors: p.visitors,
                })
                .collect(),
        })
    }
    pub async fn get_current_visitors_multi(
        &self,
        site_ids: &[u64],
    ) -> Result<MultiSiteRealtimeStats, RealtimeError> {
        let site_id_list = site_ids
            .iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(", ");

        // Count unique visitors across all sites in last 5 min
        let count_sql = format!(
            "SELECT uniq(visitor_hash) as visitors FROM events \
             WHERE site_id IN ({site_id_list}) AND timestamp >= now() - INTERVAL 5 MINUTE"
        );
        let row = self
            .client
            .query(&count_sql)
            .fetch_one::<VisitorCount>()
            .await?;

        // Per-visitor details: latest page for each visitor using argMax
        let visitors_sql = format!(
            "SELECT \
                visitor_hash, \
                argMax(country, timestamp) as last_country, \
                argMax(site_id, timestamp) as last_site_id, \
                argMax(hostname, timestamp) as last_hostname, \
                argMax(path, timestamp) as last_path \
             FROM events \
             WHERE site_id IN ({site_id_list}) AND timestamp >= now() - INTERVAL 5 MINUTE \
             GROUP BY visitor_hash \
             ORDER BY max(timestamp) DESC \
             LIMIT 100"
        );
        let visitor_rows = self
            .client
            .query(&visitors_sql)
            .fetch_all::<VisitorDetailRow>()
            .await?;

        // Top pages across all sites
        let pages_sql = format!(
            "SELECT path, uniq(visitor_hash) as visitors FROM events \
             WHERE site_id IN ({site_id_list}) AND timestamp >= now() - INTERVAL 5 MINUTE \
             GROUP BY path ORDER BY visitors DESC LIMIT 10"
        );
        let pages = self
            .client
            .query(&pages_sql)
            .fetch_all::<PageRow>()
            .await?;

        Ok(MultiSiteRealtimeStats {
            current_visitors: row.visitors,
            visitors: visitor_rows
                .into_iter()
                .map(|v| RealtimeVisitor {
                    visitor_hash: v.visitor_hash,
                    country: v.last_country,
                    site_id: v.last_site_id,
                    hostname: v.last_hostname,
                    path: v.last_path,
                })
                .collect(),
            top_pages: pages
                .into_iter()
                .map(|p| RealtimePage {
                    path: p.path,
                    visitors: p.visitors,
                })
                .collect(),
        })
    }
}

#[derive(Debug, clickhouse::Row, Deserialize)]
struct VisitorCount {
    visitors: u64,
}

#[derive(Debug, clickhouse::Row, Deserialize)]
struct PageRow {
    path: String,
    visitors: u64,
}

#[derive(Debug, clickhouse::Row, Deserialize)]
struct VisitorDetailRow {
    visitor_hash: String,
    last_country: String,
    last_site_id: u64,
    last_hostname: String,
    last_path: String,
}
