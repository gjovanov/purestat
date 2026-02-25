use bson::{doc, oid::ObjectId, DateTime};
use mongodb::Database;
use purestat_db::models::{Goal, GoalType};

use super::base::{BaseDao, DaoError, DaoResult};

pub struct GoalDao {
    pub base: BaseDao<Goal>,
}

impl GoalDao {
    pub fn new(db: &Database) -> Self {
        Self {
            base: BaseDao::new(db, Goal::COLLECTION),
        }
    }

    pub async fn create(
        &self,
        site_id: ObjectId,
        org_id: ObjectId,
        goal_type: GoalType,
        name: String,
        event_name: Option<String>,
        page_path: Option<String>,
    ) -> DaoResult<Goal> {
        let goal = Goal {
            id: None,
            site_id,
            org_id,
            goal_type,
            name,
            event_name,
            page_path,
            created_at: DateTime::now(),
        };
        let id = self.base.insert_one(&goal).await?;
        self.base
            .find_one(doc! { "_id": id })
            .await?
            .ok_or(DaoError::NotFound)
    }

    pub async fn find_by_site(&self, site_id: ObjectId) -> DaoResult<Vec<Goal>> {
        self.base
            .find_many(doc! { "site_id": site_id }, Some(doc! { "created_at": -1 }))
            .await
    }

    pub async fn delete(&self, goal_id: ObjectId) -> DaoResult<u64> {
        self.base.hard_delete(doc! { "_id": goal_id }).await
    }

    pub async fn delete_all_for_site(&self, site_id: ObjectId) -> DaoResult<u64> {
        self.base.hard_delete(doc! { "site_id": site_id }).await
    }
}
