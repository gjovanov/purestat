use purestat_api::{build_router, state::AppState};
use purestat_config::Settings;
use purestat_db::{clickhouse::connection as ch_conn, connect, indexes::ensure_indexes};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "purestat_api=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let settings = Settings::load()?;
    info!("Settings loaded");

    // Connect to MongoDB
    let db = connect(&settings).await?;
    ensure_indexes(&db).await?;

    // Connect to ClickHouse
    let ch = ch_conn::connect(&settings);

    // Connect to Redis
    let redis_client = redis::Client::open(settings.redis.url.as_str())?;
    let redis_conn = redis::aio::ConnectionManager::new(redis_client).await?;
    info!("Connected to Redis");

    // Build app state
    let app_state = AppState::new(db, ch, redis_conn, settings.clone()).await?;

    // Build router
    let app = build_router(app_state);

    // Start server
    let addr = format!("{}:{}", settings.app.host, settings.app.port);
    info!(addr = %addr, "Starting Purestat API server");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
