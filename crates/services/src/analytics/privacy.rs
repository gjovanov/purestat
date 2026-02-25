use chrono::Utc;
use hmac::{Hmac, Mac};
use redis::AsyncCommands;
use sha2::Sha256;
use thiserror::Error;
use tracing::info;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Error)]
pub enum PrivacyError {
    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),
    #[error("HMAC error: {0}")]
    Hmac(String),
}

pub struct PrivacyEngine {
    redis: redis::aio::ConnectionManager,
    salt_ttl_hours: u64,
}

impl PrivacyEngine {
    pub fn new(redis: redis::aio::ConnectionManager, salt_ttl_hours: u64) -> Self {
        Self {
            redis,
            salt_ttl_hours,
        }
    }

    pub async fn get_daily_salt(&self, date: &str) -> Result<String, PrivacyError> {
        let key = format!("purestat:salt:{date}");
        let mut conn = self.redis.clone();

        // Try to get existing salt
        let salt: Option<String> = conn.get(&key).await?;
        if let Some(s) = salt {
            return Ok(s);
        }

        // Generate new salt
        let salt = hex::encode(rand::random::<[u8; 32]>());
        let ttl_secs = self.salt_ttl_hours * 3600;

        // SET NX with TTL (atomic)
        let set: bool = redis::cmd("SET")
            .arg(&key)
            .arg(&salt)
            .arg("NX")
            .arg("EX")
            .arg(ttl_secs)
            .query_async(&mut conn)
            .await?;

        if set {
            info!(date = %date, "Generated new daily salt");
            Ok(salt)
        } else {
            // Another instance beat us — read theirs
            let existing: String = conn.get(&key).await?;
            Ok(existing)
        }
    }

    pub async fn generate_visitor_hash(
        &self,
        domain: &str,
        ip: &str,
        user_agent: &str,
    ) -> Result<String, PrivacyError> {
        let today = Utc::now().format("%Y-%m-%d").to_string();
        let salt = self.get_daily_salt(&today).await?;

        let input = format!("{salt}{domain}{ip}{user_agent}");
        let mut mac = HmacSha256::new_from_slice(salt.as_bytes())
            .map_err(|e| PrivacyError::Hmac(e.to_string()))?;
        mac.update(input.as_bytes());
        let result = mac.finalize();

        Ok(hex::encode(result.into_bytes()))
    }
}
