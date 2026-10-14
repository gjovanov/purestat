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
    #[serde(default)]
    pub email: EmailSettings,
    pub tracker: TrackerSettings,
    pub privacy: PrivacySettings,
    pub geo: GeoSettings,
    #[serde(default)]
    pub rate_limit: RateLimitSettings,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AppSettings {
    pub host: String,
    pub port: u16,
    /// The public address of purestat, where the SPA and `/api` are served
    /// (PURESTAT__APP__FRONTEND_URL).
    pub frontend_url: String,
}

impl AppSettings {
    /// Whether every cookie the API sets is `Secure`: purestat is served over
    /// https (`frontend_url`). The session cookies of a password login, a
    /// refresh, a logout and an OAuth sign-in, and the OAuth flow cookie, all
    /// follow this one rule. Over plain http (local development) none is: a
    /// browser does not keep a `Secure` cookie from an http page, localhost
    /// aside.
    pub fn secure_cookies(&self) -> bool {
        self.frontend_url
            .trim()
            .get(..8)
            .is_some_and(|scheme| scheme.eq_ignore_ascii_case("https://"))
    }
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

/// The shortest JWT secret the API accepts, in bytes.
pub const MIN_JWT_SECRET_LEN: usize = 32;

impl JwtSettings {
    /// Why `secret` must not sign tokens, or `None` when it may.
    ///
    /// The secret signs every login token, so anyone who knows it can forge a
    /// token for any user. The values this repository ships as placeholders
    /// (all starting with `change-me`) are public, which is why the API
    /// refuses to start on one rather than run with it.
    pub fn secret_problem(&self) -> Option<&'static str> {
        let secret = self.secret.as_str();
        if secret.is_empty() {
            return Some("is not set");
        }
        if secret.to_ascii_lowercase().starts_with("change-me") {
            return Some("is a placeholder published in the purestat repository");
        }
        if secret.len() < MIN_JWT_SECRET_LEN {
            return Some("is shorter than 32 bytes");
        }
        None
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct RedisSettings {
    pub url: String,
}

/// Sign-in with Google and GitHub (docs/deployment.md#oauth).
///
/// PURESTAT__OAUTH__ENABLED mounts the `/api/oauth/*` routes; off by default.
/// A provider takes part only when it is enabled AND both its client id and
/// secret are set (PURESTAT__OAUTH__GOOGLE__CLIENT_ID, ..._CLIENT_SECRET, and
/// the same under GITHUB). The callback address is not a setting: it is
/// `{app.frontend_url}/api/oauth/{provider}/callback`.
///
/// Only Google and GitHub exist. Facebook, LinkedIn and Microsoft were listed
/// once but never worked; their variables are ignored.
#[derive(Debug, Deserialize, Clone, Default)]
pub struct OAuthSettings {
    #[serde(default)]
    pub enabled: bool,
    pub google: Option<OAuthProviderSettings>,
    pub github: Option<OAuthProviderSettings>,
}

#[derive(Deserialize, Clone, Default)]
pub struct OAuthProviderSettings {
    #[serde(default)]
    pub client_id: String,
    #[serde(default)]
    pub client_secret: String,
}

impl OAuthProviderSettings {
    /// Both halves of the client credential are set.
    pub fn is_complete(&self) -> bool {
        !self.client_id.trim().is_empty() && !self.client_secret.trim().is_empty()
    }
}

/// The secret never reaches a log line through `{:?}`.
impl std::fmt::Debug for OAuthProviderSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OAuthProviderSettings")
            .field("client_id", &self.client_id)
            .field(
                "client_secret",
                &if self.client_secret.is_empty() { "" } else { "<redacted>" },
            )
            .finish()
    }
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

#[derive(Debug, Deserialize, Clone, Default)]
pub struct EmailSettings {
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_from_email")]
    pub from_email: String,
    #[serde(default = "default_from_name")]
    pub from_name: String,
    #[serde(default = "default_activation_ttl")]
    pub activation_token_ttl_minutes: u64,
}

fn default_from_email() -> String {
    "noreply@purestat.app".to_string()
}

fn default_from_name() -> String {
    "PureStat".to_string()
}

fn default_activation_ttl() -> u64 {
    5
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

/// Requests per minute per client address, by endpoint group (docs/api.md).
/// PURESTAT__RATE_LIMIT__{ENABLED,AUTH_RPM,API_RPM,TRACKER_RPM,STATS_RPM}.
#[derive(Debug, Deserialize, Clone)]
pub struct RateLimitSettings {
    #[serde(default = "default_rate_limit_enabled")]
    pub enabled: bool,
    #[serde(default = "default_auth_rpm")]
    pub auth_rpm: u32,
    #[serde(default = "default_api_rpm")]
    pub api_rpm: u32,
    #[serde(default = "default_tracker_rpm")]
    pub tracker_rpm: u32,
    #[serde(default = "default_stats_rpm")]
    pub stats_rpm: u32,
}

impl Default for RateLimitSettings {
    fn default() -> Self {
        Self {
            enabled: default_rate_limit_enabled(),
            auth_rpm: default_auth_rpm(),
            api_rpm: default_api_rpm(),
            tracker_rpm: default_tracker_rpm(),
            stats_rpm: default_stats_rpm(),
        }
    }
}

fn default_rate_limit_enabled() -> bool {
    true
}
fn default_auth_rpm() -> u32 {
    10
}
/// One dashboard load is about 15 calls, and realtime polls twice a minute.
fn default_api_rpm() -> u32 {
    300
}
fn default_tracker_rpm() -> u32 {
    1000
}
/// One multi-site dashboard load issues 12 stats calls.
fn default_stats_rpm() -> u32 {
    120
}

impl Settings {
    pub fn load() -> Result<Self, ConfigError> {
        Self::load_with(Environment::default())
    }

    /// `load`, reading the variables from `env` (a test passes a map).
    fn load_with(env: Environment) -> Result<Self, ConfigError> {
        let config = Config::builder()
            .add_source(env.separator("__").prefix("PURESTAT"))
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
            // JWT defaults. No secret: the API refuses to start until one is
            // set (JwtSettings::secret_problem).
            .set_default("jwt.secret", "")?
            .set_default("jwt.access_token_ttl_secs", 86400)?
            .set_default("jwt.refresh_token_ttl_secs", 604800)?
            .set_default("jwt.issuer", "purestat")?
            // Redis defaults
            .set_default("redis.url", "redis://localhost:6380")?
            // OAuth: the routes are not mounted unless enabled; providers are
            // None by default, and the callback address derives from
            // app.frontend_url.
            .set_default("oauth.enabled", false)?
            // Stripe defaults (empty by default — optional)
            // Email defaults
            .set_default("email.api_key", "")?
            .set_default("email.from_email", "noreply@purestat.app")?
            .set_default("email.from_name", "PureStat")?
            .set_default("email.activation_token_ttl_minutes", 5i64)?
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

#[cfg(test)]
mod tests {
    use super::*;

    fn jwt(secret: &str) -> JwtSettings {
        JwtSettings {
            secret: secret.to_string(),
            access_token_ttl_secs: 86400,
            refresh_token_ttl_secs: 604800,
            issuer: "purestat".to_string(),
        }
    }

    #[test]
    fn an_unset_secret_is_refused() {
        assert_eq!(jwt("").secret_problem(), Some("is not set"));
    }

    #[test]
    fn every_placeholder_the_repository_has_shipped_is_refused() {
        for placeholder in [
            "change-me-in-production-use-a-long-random-string", // the old code default, .env.example
            "change-me-in-production",                           // docker-compose.prod.yml's fallback
            "CHANGE-ME-to-something-long-enough-to-pass-0123456789",
        ] {
            assert!(jwt(placeholder).secret_problem().is_some(), "{placeholder}");
        }
    }

    #[test]
    fn a_short_secret_is_refused() {
        assert_eq!(
            jwt(&"x".repeat(MIN_JWT_SECRET_LEN - 1)).secret_problem(),
            Some("is shorter than 32 bytes")
        );
    }

    #[test]
    fn a_long_random_secret_is_accepted() {
        // What `openssl rand -hex 32` prints.
        let secret = "9f2c4e7a1b3d5f6e8a0c2e4f6a8b0d1e3f5a7c9e1b3d5f7a9c1e3b5d7f9a1c3e";
        assert_eq!(jwt(secret).secret_problem(), None);
        assert_eq!(jwt(&"y".repeat(MIN_JWT_SECRET_LEN)).secret_problem(), None);
    }

    /// Settings from exactly these variables, without touching the process
    /// environment (other tests in this binary read it).
    fn load_from(vars: &[(&str, &str)]) -> Settings {
        let map = vars
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        Settings::load_with(Environment::default().source(Some(map))).expect("settings load")
    }

    #[test]
    fn oauth_is_off_with_no_providers_by_default() {
        let s = load_from(&[]);
        assert!(!s.oauth.enabled);
        assert!(s.oauth.google.is_none());
        assert!(s.oauth.github.is_none());
    }

    #[test]
    fn the_documented_oauth_variables_configure_google_and_github() {
        // The names docs/deployment.md and .env.example tell an operator to set.
        let s = load_from(&[
            ("PURESTAT__OAUTH__ENABLED", "true"),
            ("PURESTAT__OAUTH__GOOGLE__CLIENT_ID", "fake-google-id"),
            ("PURESTAT__OAUTH__GOOGLE__CLIENT_SECRET", "fake-google-secret"),
            ("PURESTAT__OAUTH__GITHUB__CLIENT_ID", "fake-github-id"),
            ("PURESTAT__OAUTH__GITHUB__CLIENT_SECRET", "fake-github-secret"),
        ]);
        assert!(s.oauth.enabled);
        let google = s.oauth.google.expect("google");
        assert_eq!(google.client_id, "fake-google-id");
        assert_eq!(google.client_secret, "fake-google-secret");
        assert!(google.is_complete());
        assert!(s.oauth.github.expect("github").is_complete());
    }

    #[test]
    fn half_a_client_credential_is_not_a_configured_provider() {
        let s = load_from(&[
            ("PURESTAT__OAUTH__ENABLED", "true"),
            ("PURESTAT__OAUTH__GOOGLE__CLIENT_ID", "fake-google-id"),
            ("PURESTAT__OAUTH__GITHUB__CLIENT_SECRET", "fake-github-secret"),
        ]);
        assert!(!s.oauth.google.expect("google").is_complete());
        assert!(!s.oauth.github.expect("github").is_complete());
    }

    #[test]
    fn retired_providers_and_the_old_redirect_variable_do_not_stop_the_api() {
        // Variables the old docs listed. They must not fail the settings load,
        // and they configure nothing.
        let s = load_from(&[
            ("PURESTAT__OAUTH__FACEBOOK__CLIENT_ID", "fake-facebook-id"),
            ("PURESTAT__OAUTH__MICROSOFT__CLIENT_SECRET", "fake-ms-secret"),
            (
                "PURESTAT__OAUTH__GOOGLE__REDIRECT_URI",
                "https://elsewhere.example/api/oauth/google/callback",
            ),
        ]);
        assert!(s.oauth.github.is_none());
        assert!(!s.oauth.google.expect("google").is_complete());
    }

    #[test]
    fn cookies_are_secure_exactly_when_the_public_address_is_https() {
        let app = |url: &str| AppSettings {
            host: "0.0.0.0".to_string(),
            port: 3000,
            frontend_url: url.to_string(),
        };
        for https in [
            "https://purestat.ai",
            "https://purestat.ai/",
            "HTTPS://purestat.ai",
            " https://purestat.example:8443 ",
        ] {
            assert!(app(https).secure_cookies(), "{https:?}");
        }
        for plain in [
            "http://localhost:5173",
            "http://purestat.ai",
            "localhost:5173",
            "",
            "https:/x",
            "httpsx://purestat.ai",
        ] {
            assert!(!app(plain).secure_cookies(), "{plain:?}");
        }
        // The default is the Vite dev server, over http.
        assert!(!load_from(&[]).app.secure_cookies());
        assert!(
            load_from(&[("PURESTAT__APP__FRONTEND_URL", "https://purestat.ai")])
                .app
                .secure_cookies()
        );
    }

    #[test]
    fn a_client_secret_never_prints() {
        let p = OAuthProviderSettings {
            client_id: "fake-id".into(),
            client_secret: "fake-secret-value".into(),
        };
        let printed = format!("{p:?}");
        assert!(printed.contains("fake-id"), "{printed}");
        assert!(!printed.contains("fake-secret-value"), "{printed}");
    }
}
