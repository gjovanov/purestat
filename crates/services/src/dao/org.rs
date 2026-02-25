use bson::{doc, oid::ObjectId, DateTime};
use mongodb::Database;
use purestat_db::models::{Org, Plan, PlanLimits, Usage};

use super::base::{BaseDao, DaoResult};

pub struct OrgDao {
    pub base: BaseDao<Org>,
}

impl OrgDao {
    pub fn new(db: &Database) -> Self {
        Self {
            base: BaseDao::new(db, Org::COLLECTION),
        }
    }

    pub async fn create(
        &self,
        name: String,
        slug: String,
        owner_id: ObjectId,
    ) -> DaoResult<Org> {
        let now = DateTime::now();
        let org = Org {
            id: None,
            name,
            slug,
            owner_id,
            plan: Plan::Free,
            billing: None,
            limits: PlanLimits::for_plan(&Plan::Free),
            usage: Usage::default(),
            created_at: now,
            updated_at: now,
        };
        let id = self.base.insert_one(&org).await?;
        self.base.find_by_id(id).await
    }

    pub async fn find_by_slug(&self, slug: &str) -> DaoResult<Org> {
        self.base
            .find_one(doc! { "slug": slug })
            .await?
            .ok_or(super::base::DaoError::NotFound)
    }

    pub async fn update(
        &self,
        org_id: ObjectId,
        name: Option<String>,
    ) -> DaoResult<Org> {
        let mut set = bson::Document::new();
        if let Some(n) = name {
            set.insert("name", n);
        }
        if !set.is_empty() {
            self.base
                .update_by_id(org_id, doc! { "$set": set })
                .await?;
        }
        self.base.find_by_id(org_id).await
    }

    pub async fn update_plan(&self, org_id: ObjectId, plan: Plan) -> DaoResult<Org> {
        let limits = PlanLimits::for_plan(&plan);
        self.base
            .update_by_id(
                org_id,
                doc! {
                    "$set": {
                        "plan": bson::to_bson(&plan)?,
                        "limits": bson::to_bson(&limits)?
                    }
                },
            )
            .await?;
        self.base.find_by_id(org_id).await
    }

    pub async fn increment_pageviews(&self, org_id: ObjectId, count: u64) -> DaoResult<bool> {
        let result = self
            .base
            .collection()
            .update_one(
                doc! { "_id": org_id },
                doc! { "$inc": { "usage.current_month_pageviews": count as i64 } },
            )
            .await?;
        Ok(result.modified_count > 0)
    }

    pub async fn reset_monthly_usage(&self, org_id: ObjectId) -> DaoResult<bool> {
        self.base
            .update_by_id(
                org_id,
                doc! {
                    "$set": {
                        "usage.current_month_pageviews": 0_i64,
                        "usage.usage_reset_at": DateTime::now()
                    }
                },
            )
            .await
    }

    pub async fn delete(&self, org_id: ObjectId) -> DaoResult<u64> {
        self.base.hard_delete(doc! { "_id": org_id }).await
    }
}
