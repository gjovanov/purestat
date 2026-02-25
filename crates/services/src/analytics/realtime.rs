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
