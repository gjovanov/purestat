use bson::{doc, oid::ObjectId, DateTime};
use mongodb::Database;
use purestat_db::models::Site;

use super::base::{BaseDao, DaoError, DaoResult};

pub struct SiteDao {
    pub base: BaseDao<Site>,
}

impl SiteDao {
    pub fn new(db: &Database) -> Self {
        Self {
            base: BaseDao::new(db, Site::COLLECTION),
        }
    }

    pub async fn create(
        &self,
        org_id: ObjectId,
        domain: String,
        name: String,
        timezone: Option<String>,
    ) -> DaoResult<Site> {
        let now = DateTime::now();
        let site = Site {
            id: None,
            org_id,
            domain,
            name,
            timezone: timezone.unwrap_or_else(|| "UTC".to_string()),
            is_public: false,
            allowed_hostnames: vec![],
            created_at: now,
            updated_at: now,
        };
        let id = self.base.insert_one(&site).await?;
        self.base
            .find_one(doc! { "_id": id })
            .await?
            .ok_or(DaoError::NotFound)
    }

    pub async fn find_by_org(&self, org_id: ObjectId) -> DaoResult<Vec<Site>> {
        self.base
            .find_many(doc! { "org_id": org_id }, Some(doc! { "created_at": -1 }))
            .await
    }

    pub async fn find_by_domain(&self, domain: &str) -> DaoResult<Option<Site>> {
        self.base.find_one(doc! { "domain": domain }).await
    }

    pub async fn update(
        &self,
        site_id: ObjectId,
        name: Option<String>,
        timezone: Option<String>,
        is_public: Option<bool>,
        allowed_hostnames: Option<Vec<String>>,
    ) -> DaoResult<Site> {
        let mut set = bson::Document::new();
        if let Some(n) = name {
            set.insert("name", n);
        }
        if let Some(tz) = timezone {
            set.insert("timezone", tz);
        }
        if let Some(public) = is_public {
            set.insert("is_public", public);
        }
        if let Some(hosts) = allowed_hostnames {
            set.insert("allowed_hostnames", hosts);
        }
        if !set.is_empty() {
            self.base
                .update_by_id(site_id, doc! { "$set": set })
                .await?;
        }
        self.base
            .find_one(doc! { "_id": site_id })
            .await?
            .ok_or(DaoError::NotFound)
    }

    pub async fn delete(&self, site_id: ObjectId) -> DaoResult<u64> {
        self.base.hard_delete(doc! { "_id": site_id }).await
    }

    pub async fn count_by_org(&self, org_id: ObjectId) -> DaoResult<u64> {
        self.base.count(doc! { "org_id": org_id }).await
    }
}
