use bson::{doc, oid::ObjectId, DateTime};
use mongodb::Database;
use purestat_db::models::{Invite, InviteStatus, OrgRole};

use super::base::{BaseDao, DaoError, DaoResult};

pub struct InviteDao {
    pub base: BaseDao<Invite>,
}

impl InviteDao {
    pub fn new(db: &Database) -> Self {
        Self {
            base: BaseDao::new(db, Invite::COLLECTION),
        }
    }

    pub async fn create(
        &self,
        org_id: ObjectId,
        inviter_id: ObjectId,
        code: String,
        target_email: Option<String>,
        role: OrgRole,
        max_uses: u32,
        expires_at: DateTime,
    ) -> DaoResult<Invite> {
        let invite = Invite {
            id: None,
            org_id,
            code,
            inviter_id,
            target_email,
            role,
            max_uses,
            use_count: 0,
            expires_at,
            status: InviteStatus::Pending,
            created_at: DateTime::now(),
        };
        let id = self.base.insert_one(&invite).await?;
        self.base
            .find_one(doc! { "_id": id })
            .await?
            .ok_or(DaoError::NotFound)
    }

    pub async fn find_by_code(&self, code: &str) -> DaoResult<Invite> {
        self.base
            .find_one(doc! { "code": code })
            .await?
            .ok_or(DaoError::NotFound)
    }

    pub async fn find_by_org(&self, org_id: ObjectId) -> DaoResult<Vec<Invite>> {
        self.base
            .find_many(
                doc! { "org_id": org_id, "status": "pending" },
                Some(doc! { "created_at": -1 }),
            )
            .await
    }

    pub async fn increment_use_count(&self, invite_id: ObjectId) -> DaoResult<bool> {
        let result = self
            .base
            .collection()
            .update_one(
                doc! { "_id": invite_id },
                doc! { "$inc": { "use_count": 1 } },
            )
            .await?;
        Ok(result.modified_count > 0)
    }

    pub async fn update_status(
        &self,
        invite_id: ObjectId,
        status: InviteStatus,
    ) -> DaoResult<bool> {
        let result = self
            .base
            .collection()
            .update_one(
                doc! { "_id": invite_id },
                doc! { "$set": { "status": bson::to_bson(&status)? } },
            )
            .await?;
        Ok(result.modified_count > 0)
    }

    pub async fn delete(&self, invite_id: ObjectId) -> DaoResult<u64> {
        self.base.hard_delete(doc! { "_id": invite_id }).await
    }
}
