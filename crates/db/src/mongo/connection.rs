use mongodb::{options::ClientOptions, Client, Database};
use purestat_config::Settings;
use tracing::info;

pub async fn connect(settings: &Settings) -> Result<Database, mongodb::error::Error> {
    let mut client_options = ClientOptions::parse(&settings.database.url).await?;

    if let Some(max_pool) = settings.database.max_pool_size {
        client_options.max_pool_size = Some(max_pool);
    }
    if let Some(min_pool) = settings.database.min_pool_size {
        client_options.min_pool_size = Some(min_pool);
    }

    let client = Client::with_options(client_options)?;

    // Verify connection with ping
    client
        .database("admin")
        .run_command(bson::doc! { "ping": 1 })
        .await?;

    info!(db = %settings.database.name, "Connected to MongoDB");
    Ok(client.database(&settings.database.name))
}
