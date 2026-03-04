use mongodb::options::IndexOptions;
use mongodb::{Database, IndexModel};
use tracing::info;

pub async fn ensure_indexes(db: &Database) -> Result<(), mongodb::error::Error> {
    // Users
    create_indexes(
        db,
        "users",
        vec![
            index_unique(bson::doc! { "email": 1 }),
            index_unique(bson::doc! { "username": 1 }),
        ],
    )
    .await?;

    // Orgs
    create_indexes(
        db,
        "orgs",
        vec![
            index_unique(bson::doc! { "slug": 1 }),
            index(bson::doc! { "owner_id": 1 }),
        ],
    )
    .await?;

    // Org Members
    create_indexes(
        db,
        "org_members",
        vec![
            index_unique(bson::doc! { "org_id": 1, "user_id": 1 }),
            index(bson::doc! { "user_id": 1 }),
        ],
    )
    .await?;

    // Sites
    create_indexes(
        db,
        "sites",
        vec![
            index_unique(bson::doc! { "org_id": 1, "domain": 1 }),
            index(bson::doc! { "domain": 1 }),
        ],
    )
    .await?;

    // Goals
    create_indexes(db, "goals", vec![index(bson::doc! { "site_id": 1 })]).await?;

    // Invites
    create_indexes(
        db,
        "invites",
        vec![
            index_unique(bson::doc! { "code": 1 }),
            index(bson::doc! { "org_id": 1, "status": 1 }),
        ],
    )
    .await?;

    // Activation Codes
    create_indexes(
        db,
        "activation_codes",
        vec![
            index(bson::doc! { "user_id": 1 }),
            index(bson::doc! { "valid_to": 1 }),
        ],
    )
    .await?;

    // API Keys
    create_indexes(
        db,
        "api_keys",
        vec![
            index_unique(bson::doc! { "key_hash": 1 }),
            index(bson::doc! { "site_id": 1 }),
        ],
    )
    .await?;

    info!("MongoDB indexes ensured");
    Ok(())
}

fn index(keys: bson::Document) -> IndexModel {
    IndexModel::builder().keys(keys).build()
}

fn index_unique(keys: bson::Document) -> IndexModel {
    IndexModel::builder()
        .keys(keys)
        .options(IndexOptions::builder().unique(true).build())
        .build()
}

async fn create_indexes(
    db: &Database,
    collection: &str,
    indexes: Vec<IndexModel>,
) -> Result<(), mongodb::error::Error> {
    let coll = db.collection::<bson::Document>(collection);
    match coll.create_indexes(indexes.clone()).await {
        Ok(_) => Ok(()),
        Err(e) => {
            // Handle IndexKeySpecsConflict (code 86): drop and recreate
            if let mongodb::error::ErrorKind::Command(ref cmd_err) = *e.kind {
                if cmd_err.code == 86 {
                    coll.drop_indexes().await?;
                    coll.create_indexes(indexes).await?;
                    return Ok(());
                }
            }
            Err(e)
        }
    }
}
