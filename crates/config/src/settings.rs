use config::{Config, ConfigError, Environment};
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Settings {
    pub app: AppSettings,
    pub database: DatabaseSettings,
    pub clickhouse: ClickHouseSettings,
    pub jwt: JwtSettings,
    pub redis: RedisSettings,
    #[serde(default)]
    pub oauth: OAuthSettings,
    #[serde(default)]
    pub stripe: StripeSettings,
    pub tracker: TrackerSettings,
    pub privacy: PrivacySettings,
    pub geo: GeoSettings,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AppSettings {
    pub host: String,
    pub port: u16,
    pub frontend_url: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseSettings {
    pub url: String,
    pub name: String,
    pub max_pool_size: Option<u32>,
    pub min_pool_size: Option<u32>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ClickHouseSettings {
    pub url: String,
    pub database: String,
    pub user: String,
    pub password: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct JwtSettings {
    pub secret: String,
    pub access_token_ttl_secs: u64,
    pub refresh_token_ttl_secs: u64,
    pub issuer: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RedisSettings {
    pub url: String,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct OAuthSettings {
    pub google: Option<OAuthProviderSettings>,
    pub facebook: Option<OAuthProviderSettings>,
    pub github: Option<OAuthProviderSettings>,
    pub linkedin: Option<OAuthProviderSettings>,
    pub microsoft: Option<OAuthProviderSettings>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct OAuthProviderSettings {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct StripeSettings {
    #[serde(default)]
    pub secret_key: String,
    #[serde(default)]
    pub webhook_secret: String,
    #[serde(default)]
    pub pro_price_id: String,
    #[serde(default)]
    pub business_price_id: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TrackerSettings {
    pub host: String,
    pub port: u16,
    pub batch_size: usize,
    pub flush_interval_ms: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PrivacySettings {
    pub salt_ttl_hours: u64,
    pub session_timeout_minutes: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GeoSettings {
    pub geoip_db_path: String,
}

impl Settings {
    pub fn load() -> Result<Self, ConfigError> {
        let config = Config::builder()
            .add_source(
                Environment::default()
                    .separator("__")
                    .prefix("PURESTAT"),
            )
            // App defaults
            .set_default("app.host", "0.0.0.0")?
            .set_default("app.port", 3000)?
            .set_default("app.frontend_url", "http://localhost:5173")?
            // Database defaults
            .set_default(
                "database.url",
                "mongodb://purestat:PureStat_5ecretPa55@localhost:27020/purestat?authSource=admin",
            )?
            .set_default("database.name", "purestat")?
            // ClickHouse defaults
            .set_default("clickhouse.url", "http://localhost:8123")?
            .set_default("clickhouse.database", "purestat")?
            .set_default("clickhouse.user", "purestat")?
            .set_default("clickhouse.password", "purestat_ch_pass")?
            // JWT defaults
            .set_default("jwt.secret", "change-me-in-production-use-a-long-random-string")?
            .set_default("jwt.access_token_ttl_secs", 86400)?
            .set_default("jwt.refresh_token_ttl_secs", 604800)?
            .set_default("jwt.issuer", "purestat")?
            // Redis defaults
            .set_default("redis.url", "redis://localhost:6380")?
            // OAuth defaults (None by default — optional)
            // Stripe defaults (empty by default — optional)
            // Tracker defaults
            .set_default("tracker.host", "0.0.0.0")?
            .set_default("tracker.port", 3001)?
            .set_default("tracker.batch_size", 100)?
            .set_default("tracker.flush_interval_ms", 5000)?
            // Privacy defaults
            .set_default("privacy.salt_ttl_hours", 48)?
            .set_default("privacy.session_timeout_minutes", 30)?
            // Geo defaults
            .set_default("geo.geoip_db_path", "/data/geoip/GeoLite2-City.mmdb")?
            .build()?;

        config.try_deserialize()
    }
}
