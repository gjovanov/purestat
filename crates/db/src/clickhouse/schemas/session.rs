use clickhouse::Row;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Row, Serialize, Deserialize)]
pub struct Session {
    pub site_id: u64,
    pub visitor_hash: String,
    pub session_id: String,
    pub is_bounce: u8,
    pub entry_page: String,
    pub exit_page: String,
    pub pageviews: u32,
    pub events_count: u32,
    pub duration: u32,
    pub referrer: String,
    pub referrer_source: String,
    pub utm_source: String,
    pub utm_medium: String,
    pub utm_campaign: String,
    pub utm_content: String,
    pub utm_term: String,
    pub country: String,
    pub browser: String,
    pub os: String,
    pub device_type: String,
    #[serde(with = "clickhouse::serde::time::datetime64::millis")]
    pub started_at: time::OffsetDateTime,
    #[serde(with = "clickhouse::serde::time::datetime64::millis")]
    pub ended_at: time::OffsetDateTime,
}
