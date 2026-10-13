use clickhouse::Client;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::scope::{SiteScope, events_scope};
use super::sql::Sql;

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

/// The visitors of the last five minutes: those with a pageview on one of
/// the site's hostnames in that time.
pub struct RealtimeService {
    client: Client,
}

/// The pageviews of the last five minutes on these sites' hostnames.
fn recent_pageviews(scopes: &[SiteScope]) -> Sql {
    let mut sql = events_scope(scopes);
    sql.push(" AND event_name = 'pageview' AND timestamp >= now() - INTERVAL 5 MINUTE");
    sql
}

impl RealtimeService {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    pub async fn get_current_visitors(
        &self,
        scope: &SiteScope,
    ) -> Result<RealtimeStats, RealtimeError> {
        let scopes = std::slice::from_ref(scope);
        let mut count_sql = Sql::new("SELECT uniq(visitor_hash) AS visitors FROM events WHERE ");
        count_sql.append(&recent_pageviews(scopes));
        let row = count_sql
            .query(&self.client)
            .fetch_one::<VisitorCount>()
            .await?;

        let pages = self.top_pages(scopes).await?;
        Ok(RealtimeStats {
            current_visitors: row.visitors,
            top_pages: pages,
        })
    }

    pub async fn get_current_visitors_multi(
        &self,
        scopes: &[SiteScope],
    ) -> Result<MultiSiteRealtimeStats, RealtimeError> {
        let mut count_sql = Sql::new("SELECT uniq(visitor_hash) AS visitors FROM events WHERE ");
        count_sql.append(&recent_pageviews(scopes));
        let row = count_sql
            .query(&self.client)
            .fetch_one::<VisitorCount>()
            .await?;

        // Per-visitor details: the latest page of each visitor.
        let mut visitors_sql = Sql::new(
            "SELECT \
                visitor_hash, \
                argMax(country, timestamp) AS last_country, \
                argMax(site_id, timestamp) AS last_site_id, \
                argMax(hostname, timestamp) AS last_hostname, \
                argMax(path, timestamp) AS last_path \
             FROM events WHERE ",
        );
        visitors_sql.append(&recent_pageviews(scopes));
        visitors_sql.push(" GROUP BY visitor_hash ORDER BY max(timestamp) DESC LIMIT 100");
        let visitor_rows = visitors_sql
            .query(&self.client)
            .fetch_all::<VisitorDetailRow>()
            .await?;

        let pages = self.top_pages(scopes).await?;
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
            top_pages: pages,
        })
    }

    async fn top_pages(&self, scopes: &[SiteScope]) -> Result<Vec<RealtimePage>, RealtimeError> {
        let mut sql = Sql::new("SELECT path, uniq(visitor_hash) AS visitors FROM events WHERE ");
        sql.append(&recent_pageviews(scopes));
        sql.push(" GROUP BY path ORDER BY visitors DESC, path ASC LIMIT 10");
        let pages = sql.query(&self.client).fetch_all::<PageRow>().await?;
        Ok(pages
            .into_iter()
            .map(|p| RealtimePage {
                path: p.path,
                visitors: p.visitors,
            })
            .collect())
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
