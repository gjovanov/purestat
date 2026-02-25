-- Purestat ClickHouse Schema
-- Events table: all tracked events (pageviews are name='pageview')
CREATE TABLE IF NOT EXISTS events (
    site_id       UInt64,
    visitor_hash  String,
    session_id    String,
    event_name    String,
    url           String,
    path          String,
    hostname      String,
    referrer      String,
    referrer_source String,
    utm_source    String,
    utm_medium    String,
    utm_campaign  String,
    utm_content   String,
    utm_term      String,
    country       LowCardinality(String),
    region        String,
    city          String,
    browser       LowCardinality(String),
    browser_version String,
    os            LowCardinality(String),
    os_version    String,
    device_type   LowCardinality(String),
    screen_width  UInt16,
    screen_height UInt16,
    prop_keys     Array(String),
    prop_values   Array(String),
    revenue_amount  Nullable(Float64),
    revenue_currency Nullable(String),
    timestamp     DateTime64(3, 'UTC'),
    date          Date MATERIALIZED toDate(timestamp)
) ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (site_id, date, visitor_hash, session_id, timestamp)
TTL date + INTERVAL 2 YEAR;

-- Sessions table: aggregated session-level data
CREATE TABLE IF NOT EXISTS sessions (
    site_id        UInt64,
    visitor_hash   String,
    session_id     String,
    is_bounce      UInt8,
    entry_page     String,
    exit_page      String,
    pageviews      UInt32,
    events_count   UInt32,
    duration       UInt32,
    referrer       String,
    referrer_source String,
    utm_source     String,
    utm_medium     String,
    utm_campaign   String,
    utm_content    String,
    utm_term       String,
    country        LowCardinality(String),
    browser        LowCardinality(String),
    os             LowCardinality(String),
    device_type    LowCardinality(String),
    started_at     DateTime64(3, 'UTC'),
    ended_at       DateTime64(3, 'UTC'),
    date           Date MATERIALIZED toDate(started_at)
) ENGINE = MergeTree()
PARTITION BY toYYYYMM(started_at)
ORDER BY (site_id, date, visitor_hash, session_id)
TTL date + INTERVAL 2 YEAR;
