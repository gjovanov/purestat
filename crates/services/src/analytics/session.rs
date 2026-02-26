use clickhouse::Client;
use purestat_db::clickhouse::schemas::Session;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;
use tracing::{error, info};

#[derive(Debug, Error)]
pub enum SessionError {
    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),
    #[error("ClickHouse error: {0}")]
    ClickHouse(#[from] clickhouse::error::Error),
    #[error("Serde error: {0}")]
    Serde(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    pub site_id: u64,
    pub visitor_hash: String,
    pub session_id: String,
    pub entry_page: String,
    pub exit_page: String,
    pub pageviews: u32,
    pub events_count: u32,
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
    pub started_at: i64,
    pub last_event_at: i64,
}

pub struct EventSessionData {
    pub site_id: u64,
    pub visitor_hash: String,
    pub event_name: String,
    pub path: String,
    pub referrer: String,
    pub referrer_source: String,
    pub country: String,
    pub browser: String,
    pub os: String,
    pub device_type: String,
}

pub struct SessionService {
    redis: redis::aio::ConnectionManager,
    ch_client: Client,
    session_timeout_minutes: u64,
}

impl SessionService {
    pub fn new(
        redis: redis::aio::ConnectionManager,
        ch_client: Client,
        session_timeout_minutes: u64,
    ) -> Self {
        Self {
            redis,
            ch_client,
            session_timeout_minutes,
        }
    }

    /// Process an event and return the session_id to use.
    pub async fn track_event(&self, data: &EventSessionData) -> Result<String, SessionError> {
        let visitor_prefix = &data.visitor_hash[..16.min(data.visitor_hash.len())];
        let lookup_key = format!("purestat:vs:{}:{}", data.site_id, visitor_prefix);
        let mut conn = self.redis.clone();

        // TTL includes 5-min buffer so sweeper can read data before expiry
        let ttl_secs = (self.session_timeout_minutes + 5) * 60;

        // Check for active session
        let existing_session_key: Option<String> = conn.get(&lookup_key).await?;

        if let Some(session_key) = existing_session_key {
            if let Some(json) = conn.get::<_, Option<String>>(&session_key).await? {
                let mut state: SessionState = serde_json::from_str(&json)?;
                state.exit_page = data.path.clone();
                state.events_count += 1;
                if data.event_name == "pageview" {
                    state.pageviews += 1;
                }
                state.last_event_at = chrono::Utc::now().timestamp_millis();
                if state.country.is_empty() && !data.country.is_empty() {
                    state.country = data.country.clone();
                }

                let updated_json = serde_json::to_string(&state)?;
                conn.set_ex::<_, _, ()>(&session_key, &updated_json, ttl_secs)
                    .await?;
                conn.set_ex::<_, _, ()>(&lookup_key, &session_key, ttl_secs)
                    .await?;
                return Ok(state.session_id);
            }
        }

        // Create new session
        let now = chrono::Utc::now();
        let session_id = format!("{}-{}", visitor_prefix, now.timestamp());
        let session_key = format!("purestat:s:{}", session_id);

        let state = SessionState {
            site_id: data.site_id,
            visitor_hash: data.visitor_hash.clone(),
            session_id: session_id.clone(),
            entry_page: data.path.clone(),
            exit_page: data.path.clone(),
            pageviews: if data.event_name == "pageview" { 1 } else { 0 },
            events_count: 1,
            referrer: data.referrer.clone(),
            referrer_source: data.referrer_source.clone(),
            utm_source: String::new(),
            utm_medium: String::new(),
            utm_campaign: String::new(),
            utm_content: String::new(),
            utm_term: String::new(),
            country: data.country.clone(),
            browser: data.browser.clone(),
            os: data.os.clone(),
            device_type: data.device_type.clone(),
            started_at: now.timestamp_millis(),
            last_event_at: now.timestamp_millis(),
        };

        let json = serde_json::to_string(&state)?;
        conn.set_ex::<_, _, ()>(&session_key, &json, ttl_secs)
            .await?;
        conn.set_ex::<_, _, ()>(&lookup_key, &session_key, ttl_secs)
            .await?;

        Ok(session_id)
    }

    /// Background sweeper: find expired sessions and flush to ClickHouse.
    pub fn start_session_sweeper(self: &Arc<Self>, interval_secs: u64) {
        let svc = Arc::clone(self);
        tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(tokio::time::Duration::from_secs(interval_secs));
            loop {
                interval.tick().await;
                if let Err(e) = svc.sweep_expired_sessions().await {
                    error!(error = %e, "Session sweep failed");
                }
            }
        });
    }

    async fn sweep_expired_sessions(&self) -> Result<(), SessionError> {
        let mut conn = self.redis.clone();
        let timeout_ms = (self.session_timeout_minutes * 60 * 1000) as i64;
        let now = chrono::Utc::now().timestamp_millis();

        let mut cursor: u64 = 0;
        let mut sessions_to_flush: Vec<SessionState> = Vec::new();
        let mut keys_to_delete: Vec<String> = Vec::new();

        loop {
            let (next_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg("purestat:s:*")
                .arg("COUNT")
                .arg(100)
                .query_async(&mut conn)
                .await?;

            for key in keys {
                if let Some(json) = conn.get::<_, Option<String>>(&key).await? {
                    if let Ok(state) = serde_json::from_str::<SessionState>(&json) {
                        if now - state.last_event_at > timeout_ms {
                            sessions_to_flush.push(state);
                            keys_to_delete.push(key);
                        }
                    }
                }
            }

            cursor = next_cursor;
            if cursor == 0 {
                break;
            }
        }

        if !sessions_to_flush.is_empty() {
            let count = sessions_to_flush.len();
            let mut inserter = self.ch_client.insert("sessions")?;
            for state in &sessions_to_flush {
                let started_at = time::OffsetDateTime::from_unix_timestamp_nanos(
                    state.started_at as i128 * 1_000_000,
                )
                .unwrap_or(time::OffsetDateTime::now_utc());
                let ended_at = time::OffsetDateTime::from_unix_timestamp_nanos(
                    state.last_event_at as i128 * 1_000_000,
                )
                .unwrap_or(time::OffsetDateTime::now_utc());

                let session = Session {
                    site_id: state.site_id,
                    visitor_hash: state.visitor_hash.clone(),
                    session_id: state.session_id.clone(),
                    is_bounce: if state.pageviews <= 1 { 1 } else { 0 },
                    entry_page: state.entry_page.clone(),
                    exit_page: state.exit_page.clone(),
                    pageviews: state.pageviews,
                    events_count: state.events_count,
                    duration: ((state.last_event_at - state.started_at) / 1000) as u32,
                    referrer: state.referrer.clone(),
                    referrer_source: state.referrer_source.clone(),
                    utm_source: state.utm_source.clone(),
                    utm_medium: state.utm_medium.clone(),
                    utm_campaign: state.utm_campaign.clone(),
                    utm_content: state.utm_content.clone(),
                    utm_term: state.utm_term.clone(),
                    country: state.country.clone(),
                    browser: state.browser.clone(),
                    os: state.os.clone(),
                    device_type: state.device_type.clone(),
                    started_at,
                    ended_at,
                };
                inserter.write(&session).await?;
            }
            inserter.end().await?;

            // Delete flushed sessions and their lookup keys
            for key in &keys_to_delete {
                conn.del::<_, ()>(key).await?;
            }
            for state in &sessions_to_flush {
                let prefix = &state.visitor_hash[..16.min(state.visitor_hash.len())];
                let lookup_key = format!("purestat:vs:{}:{}", state.site_id, prefix);
                // Only delete lookup if it still points to this (now-flushed) session
                if let Some(val) = conn.get::<_, Option<String>>(&lookup_key).await? {
                    let expected = format!("purestat:s:{}", state.session_id);
                    if val == expected {
                        conn.del::<_, ()>(&lookup_key).await?;
                    }
                }
            }

            info!(count = count, "Flushed expired sessions to ClickHouse");
        }

        Ok(())
    }
}
