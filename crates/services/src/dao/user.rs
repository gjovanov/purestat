use bson::{doc, oid::ObjectId, DateTime};
use futures::TryStreamExt;
use mongodb::options::{Collation, CollationStrength};
use mongodb::Database;
use purestat_db::models::{OAuthProvider, User};

use super::base::{BaseDao, DaoError, DaoResult};

pub struct UserDao {
    pub base: BaseDao<User>,
}

impl UserDao {
    pub fn new(db: &Database) -> Self {
        Self {
            base: BaseDao::new(db, User::COLLECTION),
        }
    }

    pub async fn create(
        &self,
        email: String,
        username: String,
        display_name: String,
        password_hash: Option<String>,
    ) -> DaoResult<User> {
        let now = DateTime::now();
        let user = User {
            id: None,
            email,
            username,
            display_name,
            avatar: None,
            password_hash,
            oauth_providers: vec![],
            is_verified: false,
            // Unproven until the emailed activation link is followed.
            email_proven: Some(false),
            locale: "en".to_string(),
            created_at: now,
            updated_at: now,
            deleted_at: None,
        };
        let id = self.base.insert_one(&user).await?;
        self.base.find_by_id(id).await
    }

    pub async fn find_by_email(&self, email: &str) -> DaoResult<User> {
        self.base
            .find_one(doc! { "email": email, "deleted_at": null })
            .await?
            .ok_or(DaoError::NotFound)
    }

    pub async fn find_by_username(&self, username: &str) -> DaoResult<User> {
        self.base
            .find_one(doc! { "username": username, "deleted_at": null })
            .await?
            .ok_or(DaoError::NotFound)
    }

    pub async fn find_by_oauth(
        &self,
        provider: &str,
        provider_id: &str,
    ) -> DaoResult<Option<User>> {
        self.base
            .find_one(doc! {
                "oauth_providers": {
                    "$elemMatch": {
                        "provider": provider,
                        "provider_id": provider_id
                    }
                },
                "deleted_at": null
            })
            .await
    }

    /// Live accounts whose address equals `email` ignoring letter case, at
    /// most `limit` of them. Registration stores an address as typed, while a
    /// provider may report it in another case.
    pub async fn find_by_email_ignoring_case(
        &self,
        email: &str,
        limit: i64,
    ) -> DaoResult<Vec<User>> {
        let ignore_case = Collation::builder()
            .locale("en")
            .strength(CollationStrength::Secondary)
            .build();
        let cursor = self
            .base
            .collection()
            .find(doc! { "email": email, "deleted_at": null })
            .collation(ignore_case)
            .limit(limit)
            .await?;
        Ok(cursor.try_collect().await?)
    }

    /// A new account for an identity Google or GitHub vouched for: activated,
    /// its address proven (the provider verified it), no password, and the
    /// identity linked.
    pub async fn create_oauth(
        &self,
        email: String,
        username: String,
        display_name: String,
        identity: OAuthProvider,
    ) -> DaoResult<User> {
        let now = DateTime::now();
        let user = User {
            id: None,
            email,
            username,
            display_name,
            avatar: None,
            password_hash: None,
            oauth_providers: vec![identity],
            is_verified: true,
            email_proven: Some(true),
            locale: "en".to_string(),
            created_at: now,
            updated_at: now,
            deleted_at: None,
        };
        let id = self.base.insert_one(&user).await?;
        self.base.find_by_id(id).await
    }

    /// Link `identity` to a live account. Only for an account that proved
    /// its address, which the provider has now vouched for as well. False
    /// when the account no longer exists.
    pub async fn link_oauth(&self, user_id: ObjectId, identity: &OAuthProvider) -> DaoResult<bool> {
        let result = self
            .base
            .collection()
            .update_one(
                doc! { "_id": user_id, "deleted_at": null },
                doc! {
                    "$addToSet": { "oauth_providers": bson::to_bson(identity)? },
                    "$set": { "email_proven": true, "updated_at": DateTime::now() }
                },
            )
            .await?;
        Ok(result.matched_count > 0)
    }

    /// Activate an account, so it may sign in with its password.
    /// `email_proven` says whether that came with proof the address is the
    /// registrant's (the emailed link) or without it (no email service).
    pub async fn activate(&self, user_id: ObjectId, email_proven: bool) -> DaoResult<bool> {
        let result = self
            .base
            .collection()
            .update_one(
                doc! { "_id": user_id },
                doc! { "$set": {
                    "is_verified": true,
                    "email_proven": email_proven,
                    "updated_at": DateTime::now(),
                } },
            )
            .await?;
        Ok(result.matched_count > 0)
    }

    pub async fn update_profile(
        &self,
        user_id: ObjectId,
        display_name: Option<String>,
        avatar: Option<String>,
        locale: Option<String>,
    ) -> DaoResult<User> {
        let mut set = bson::Document::new();
        if let Some(name) = display_name {
            set.insert("display_name", name);
        }
        if let Some(av) = avatar {
            set.insert("avatar", av);
        }
        if let Some(loc) = locale {
            set.insert("locale", loc);
        }
        if !set.is_empty() {
            self.base
                .update_by_id(user_id, doc! { "$set": set })
                .await?;
        }
        self.base.find_by_id(user_id).await
    }
}
