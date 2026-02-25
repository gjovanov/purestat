use purestat_config::Settings;
use purestat_db::clickhouse::connection as ch_conn;
use purestat_services::analytics::ingest::IngestService;
use purestat_services::analytics::privacy::PrivacyEngine;
use purestat_tracker::{build_tracker_router, TrackerState};
use std::sync::Arc;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "purestat_tracker=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let settings = Settings::load()?;
    info!("Settings loaded");

    // Connect to ClickHouse
    let ch = ch_conn::connect(&settings);

    // Connect to Redis
    let redis_client = redis::Client::open(settings.redis.url.as_str())?;
    let redis_conn = redis::aio::ConnectionManager::new(redis_client).await?;
    info!("Connected to Redis");

    // Build services
    let ingest = Arc::new(IngestService::new(ch, settings.tracker.batch_size));
    ingest.start_flush_timer(settings.tracker.flush_interval_ms);

    let privacy = Arc::new(PrivacyEngine::new(
        redis_conn,
        settings.privacy.salt_ttl_hours,
    ));

    let state = TrackerState { ingest, privacy };
    let app = build_tracker_router(state);

    let addr = format!("{}:{}", settings.tracker.host, settings.tracker.port);
    info!(addr = %addr, "Starting Purestat Tracker server");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
