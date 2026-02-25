use bson::{doc, oid::ObjectId, DateTime};
use mongodb::Database;
use purestat_db::models::{OrgMember, OrgRole};

use super::base::{BaseDao, DaoError, DaoResult};

pub struct OrgMemberDao {
    pub base: BaseDao<OrgMember>,
}

impl OrgMemberDao {
    pub fn new(db: &Database) -> Self {
        Self {
            base: BaseDao::new(db, OrgMember::COLLECTION),
        }
    }

    pub async fn create(
        &self,
        org_id: ObjectId,
        user_id: ObjectId,
        role: OrgRole,
        invited_by: Option<ObjectId>,
    ) -> DaoResult<OrgMember> {
        let member = OrgMember {
            id: None,
            org_id,
            user_id,
            role,
            joined_at: DateTime::now(),
            invited_by,
        };
        let id = self.base.insert_one(&member).await?;
        self.base
            .find_one(doc! { "_id": id })
            .await?
            .ok_or(DaoError::NotFound)
    }

    pub async fn find_by_org(&self, org_id: ObjectId) -> DaoResult<Vec<OrgMember>> {
        self.base
            .find_many(doc! { "org_id": org_id }, None)
            .await
    }

    pub async fn find_by_user(&self, user_id: ObjectId) -> DaoResult<Vec<OrgMember>> {
        self.base
            .find_many(doc! { "user_id": user_id }, None)
            .await
    }

    pub async fn find_membership(
        &self,
        org_id: ObjectId,
        user_id: ObjectId,
    ) -> DaoResult<OrgMember> {
        self.base
            .find_one(doc! { "org_id": org_id, "user_id": user_id })
            .await?
            .ok_or(DaoError::NotFound)
    }

    pub async fn update_role(
        &self,
        member_id: ObjectId,
        role: OrgRole,
    ) -> DaoResult<bool> {
        let result = self
            .base
            .collection()
            .update_one(
                doc! { "_id": member_id },
                doc! { "$set": { "role": bson::to_bson(&role)? } },
            )
            .await?;
        Ok(result.modified_count > 0)
    }

    pub async fn remove(&self, member_id: ObjectId) -> DaoResult<u64> {
        self.base.hard_delete(doc! { "_id": member_id }).await
    }

    pub async fn remove_all_for_org(&self, org_id: ObjectId) -> DaoResult<u64> {
        self.base.hard_delete(doc! { "org_id": org_id }).await
    }
}
