use bson::{DateTime, doc, oid::ObjectId};
use mongodb::Database;
use purestat_db::models::{Funnel, FunnelStep};

use super::base::{BaseDao, DaoError, DaoResult};

/// Funnels, always addressed within their site: an id from another site
/// finds, changes and deletes nothing (GHSA-7f5h-5qwr-rxh5).
pub struct FunnelDao {
    pub base: BaseDao<Funnel>,
}

impl FunnelDao {
    pub fn new(db: &Database) -> Self {
        Self {
            base: BaseDao::new(db, Funnel::COLLECTION),
        }
    }

    pub async fn create(
        &self,
        site_id: ObjectId,
        org_id: ObjectId,
        name: String,
        steps: Vec<FunnelStep>,
    ) -> DaoResult<Funnel> {
        let now = DateTime::now();
        let funnel = Funnel {
            id: None,
            site_id,
            org_id,
            name,
            steps,
            created_at: now,
            updated_at: now,
        };
        let id = self.base.insert_one(&funnel).await?;
        self.find(id, site_id).await?.ok_or(DaoError::NotFound)
    }

    pub async fn find_by_site(&self, site_id: ObjectId) -> DaoResult<Vec<Funnel>> {
        self.base
            .find_many(doc! { "site_id": site_id }, Some(doc! { "created_at": -1 }))
            .await
    }

    /// The funnel, if it belongs to `site_id`.
    pub async fn find(&self, funnel_id: ObjectId, site_id: ObjectId) -> DaoResult<Option<Funnel>> {
        self.base
            .find_one(doc! { "_id": funnel_id, "site_id": site_id })
            .await
    }

    /// Replaces the name and the steps of the funnel if it belongs to
    /// `site_id`, and returns it as stored; `None` when there is no such
    /// funnel on the site.
    pub async fn update(
        &self,
        funnel_id: ObjectId,
        site_id: ObjectId,
        name: String,
        steps: Vec<FunnelStep>,
    ) -> DaoResult<Option<Funnel>> {
        let steps = bson::to_bson(&steps)?;
        let matched = self
            .base
            .collection()
            .update_one(
                doc! { "_id": funnel_id, "site_id": site_id },
                doc! { "$set": { "name": name, "steps": steps, "updated_at": DateTime::now() } },
            )
            .await?
            .matched_count;
        if matched == 0 {
            return Ok(None);
        }
        self.find(funnel_id, site_id).await
    }

    /// Deletes the funnel only if it belongs to `site_id`.
    pub async fn delete(&self, funnel_id: ObjectId, site_id: ObjectId) -> DaoResult<u64> {
        self.base
            .hard_delete(doc! { "_id": funnel_id, "site_id": site_id })
            .await
    }

    pub async fn delete_all_for_site(&self, site_id: ObjectId) -> DaoResult<u64> {
        self.base.hard_delete(doc! { "site_id": site_id }).await
    }
}
