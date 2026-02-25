use clickhouse::Row;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Row, Serialize, Deserialize)]
pub struct Event {
    pub site_id: u64,
    pub visitor_hash: String,
    pub session_id: String,
    pub event_name: String,
    pub url: String,
    pub path: String,
    pub hostname: String,
    pub referrer: String,
    pub referrer_source: String,
    pub utm_source: String,
    pub utm_medium: String,
    pub utm_campaign: String,
    pub utm_content: String,
    pub utm_term: String,
    pub country: String,
    pub region: String,
    pub city: String,
    pub browser: String,
    pub browser_version: String,
    pub os: String,
    pub os_version: String,
    pub device_type: String,
    pub screen_width: u16,
    pub screen_height: u16,
    pub prop_keys: Vec<String>,
    pub prop_values: Vec<String>,
    pub revenue_amount: Option<f64>,
    pub revenue_currency: Option<String>,
    #[serde(with = "clickhouse::serde::time::datetime64::millis")]
    pub timestamp: time::OffsetDateTime,
}
