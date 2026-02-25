use clickhouse::Client;
use purestat_config::Settings;
use tracing::info;

pub fn connect(settings: &Settings) -> Client {
    let client = Client::default()
        .with_url(&settings.clickhouse.url)
        .with_user(&settings.clickhouse.user)
        .with_password(&settings.clickhouse.password)
        .with_database(&settings.clickhouse.database);

    info!(
        url = %settings.clickhouse.url,
        db = %settings.clickhouse.database,
        "Configured ClickHouse client"
    );

    client
}
