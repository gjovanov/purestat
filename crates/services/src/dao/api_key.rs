use bson::{doc, oid::ObjectId, DateTime};
use mongodb::Database;
use purestat_db::models::ApiKey;

use super::base::{BaseDao, DaoError, DaoResult};

pub struct ApiKeyDao {
    pub base: BaseDao<ApiKey>,
}

impl ApiKeyDao {
    pub fn new(db: &Database) -> Self {
        Self {
            base: BaseDao::new(db, ApiKey::COLLECTION),
        }
    }

    pub async fn create(
        &self,
        site_id: ObjectId,
        org_id: ObjectId,
        name: String,
        key_hash: String,
        key_prefix: String,
        scopes: Vec<String>,
    ) -> DaoResult<ApiKey> {
        let api_key = ApiKey {
            id: None,
            site_id,
            org_id,
            name,
            key_hash,
            key_prefix,
            scopes,
            last_used_at: None,
            created_at: DateTime::now(),
            revoked_at: None,
        };
        let id = self.base.insert_one(&api_key).await?;
        self.base
            .find_one(doc! { "_id": id })
            .await?
            .ok_or(DaoError::NotFound)
    }

    pub async fn find_by_hash(&self, key_hash: &str) -> DaoResult<Option<ApiKey>> {
        self.base
            .find_one(doc! { "key_hash": key_hash, "revoked_at": null })
            .await
    }

    pub async fn find_by_site(&self, site_id: ObjectId) -> DaoResult<Vec<ApiKey>> {
        self.base
            .find_many(doc! { "site_id": site_id }, Some(doc! { "created_at": -1 }))
            .await
    }

    pub async fn revoke(&self, key_id: ObjectId) -> DaoResult<bool> {
        let result = self
            .base
            .collection()
            .update_one(
                doc! { "_id": key_id, "revoked_at": null },
                doc! { "$set": { "revoked_at": DateTime::now() } },
            )
            .await?;
        Ok(result.modified_count > 0)
    }

    pub async fn touch_last_used(&self, key_id: ObjectId) -> DaoResult<bool> {
        let result = self
            .base
            .collection()
            .update_one(
                doc! { "_id": key_id },
                doc! { "$set": { "last_used_at": DateTime::now() } },
            )
            .await?;
        Ok(result.modified_count > 0)
    }
}
