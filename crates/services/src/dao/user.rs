use bson::{doc, oid::ObjectId, DateTime};
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

    pub async fn add_oauth_provider(
        &self,
        user_id: ObjectId,
        provider: OAuthProvider,
    ) -> DaoResult<bool> {
        let result = self
            .base
            .collection()
            .update_one(
                doc! { "_id": user_id },
                doc! {
                    "$push": { "oauth_providers": bson::to_bson(&provider)? },
                    "$set": { "updated_at": DateTime::now() }
                },
            )
            .await?;
        Ok(result.modified_count > 0)
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
