use bson::{doc, oid::ObjectId, DateTime};
use mongodb::Database;
use purestat_db::models::Site;

use super::base::{BaseDao, DaoError, DaoResult};

pub struct SiteDao {
    pub base: BaseDao<Site>,
}

/// How many ids a creation draws before it gives up; a second draw is
/// already only for a race between two creations.
const CH_ID_ATTEMPTS: usize = 8;

impl SiteDao {
    pub fn new(db: &Database) -> Self {
        Self {
            base: BaseDao::new(db, Site::COLLECTION),
        }
    }

    /// Creates the site with a ClickHouse id of its own (`Site::ch_id`).
    pub async fn create(
        &self,
        org_id: ObjectId,
        domain: String,
        name: String,
        timezone: Option<String>,
    ) -> DaoResult<Site> {
        let now = DateTime::now();
        let mut site = Site {
            id: None,
            org_id,
            domain,
            name,
            timezone: timezone.unwrap_or_else(|| "UTC".to_string()),
            is_public: false,
            allowed_hostnames: vec![],
            ch_id: None,
            created_at: now,
            updated_at: now,
        };
        // The unique index on `ch_id` is the last word: when another creation
        // took the id first, this one draws again.
        for _ in 0..CH_ID_ATTEMPTS {
            site.ch_id = Some(self.unused_ch_id().await?);
            match self.base.insert_one(&site).await {
                Ok(id) => {
                    return self
                        .base
                        .find_one(doc! { "_id": id })
                        .await?
                        .ok_or(DaoError::NotFound);
                }
                Err(DaoError::DuplicateKey(msg)) if msg.contains("ch_id") => continue,
                Err(e) => return Err(e),
            }
        }
        Err(DaoError::Validation(
            "Could not assign the site an analytics id".to_string(),
        ))
    }

    /// A random ClickHouse id that no site has: neither as its `ch_id`, nor
    /// as the id derived from the ObjectId of a site without one. Positive
    /// and within i64, because BSON stores it as an Int64.
    async fn unused_ch_id(&self) -> DaoResult<u64> {
        loop {
            let candidate = rand::random::<u64>() & (i64::MAX as u64);
            if candidate == 0 {
                continue;
            }
            let prefix = candidate.to_be_bytes();
            let mut low = [0u8; 12];
            let mut high = [0xffu8; 12];
            low[..8].copy_from_slice(&prefix);
            high[..8].copy_from_slice(&prefix);
            let taken = self
                .base
                .count(doc! {
                    "$or": [
                        { "ch_id": candidate as i64 },
                        {
                            "ch_id": { "$exists": false },
                            "_id": { "$gte": ObjectId::from_bytes(low), "$lte": ObjectId::from_bytes(high) },
                        },
                    ]
                })
                .await?;
            if taken == 0 {
                return Ok(candidate);
            }
        }
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
