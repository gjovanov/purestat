use bson::{doc, oid::ObjectId, DateTime, Document};
use mongodb::Collection;
use mongodb::Database;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DaoError {
    #[error("MongoDB error: {0}")]
    Mongo(#[from] mongodb::error::Error),
    #[error("BSON serialization error: {0}")]
    BsonSer(#[from] bson::ser::Error),
    #[error("BSON deserialization error: {0}")]
    BsonDe(#[from] bson::de::Error),
    #[error("Entity not found")]
    NotFound,
    #[error("Duplicate key: {0}")]
    DuplicateKey(String),
    #[error("Forbidden: {0}")]
    Forbidden(String),
    #[error("Validation: {0}")]
    Validation(String),
}

pub type DaoResult<T> = Result<T, DaoError>;

fn default_page() -> u64 {
    1
}
fn default_per_page() -> u64 {
    20
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationParams {
    #[serde(default = "default_page")]
    pub page: u64,
    #[serde(default = "default_per_page")]
    pub per_page: u64,
}

impl Default for PaginationParams {
    fn default() -> Self {
        Self {
            page: default_page(),
            per_page: default_per_page(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedResult<T> {
    pub items: Vec<T>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
    pub total_pages: u64,
}

pub struct BaseDao<T: Send + Sync> {
    collection: Collection<T>,
}

impl<T> BaseDao<T>
where
    T: Serialize + for<'de> Deserialize<'de> + Unpin + Send + Sync,
{
    pub fn new(db: &Database, collection_name: &str) -> Self {
        Self {
            collection: db.collection::<T>(collection_name),
        }
    }

    pub fn collection(&self) -> &Collection<T> {
        &self.collection
    }

    pub async fn find_by_id(&self, id: ObjectId) -> DaoResult<T> {
        self.collection
            .find_one(doc! { "_id": id, "deleted_at": null })
            .await?
            .ok_or(DaoError::NotFound)
    }

    pub async fn find_one(&self, filter: Document) -> DaoResult<Option<T>> {
        Ok(self.collection.find_one(filter).await?)
    }

    pub async fn find_many(
        &self,
        filter: Document,
        sort: Option<Document>,
    ) -> DaoResult<Vec<T>> {
        use futures::TryStreamExt;
        let mut find = self.collection.find(filter);
        if let Some(sort_doc) = sort {
            find = find.sort(sort_doc);
        }
        let cursor = find.await?;
        let results: Vec<T> = cursor.try_collect().await?;
        Ok(results)
    }

    pub async fn find_paginated(
        &self,
        filter: Document,
        sort: Option<Document>,
        params: &PaginationParams,
    ) -> DaoResult<PaginatedResult<T>> {
        use futures::TryStreamExt;

        let total = self.collection.count_documents(filter.clone()).await?;
        let skip = (params.page.saturating_sub(1)) * params.per_page;

        let mut find = self.collection.find(filter).skip(skip).limit(params.per_page as i64);
        if let Some(sort_doc) = sort {
            find = find.sort(sort_doc);
        }
        let cursor = find.await?;
        let items: Vec<T> = cursor.try_collect().await?;

        let total_pages = if total == 0 {
            0
        } else {
            (total + params.per_page - 1) / params.per_page
        };

        Ok(PaginatedResult {
            items,
            total,
            page: params.page,
            per_page: params.per_page,
            total_pages,
        })
    }

    pub async fn insert_one(&self, doc: &T) -> DaoResult<ObjectId> {
        match self.collection.insert_one(doc).await {
            Ok(result) => Ok(result
                .inserted_id
                .as_object_id()
                .expect("inserted_id should be ObjectId")),
            Err(e) => {
                if let mongodb::error::ErrorKind::Write(mongodb::error::WriteFailure::WriteError(
                    ref write_err,
                )) = *e.kind
                {
                    if write_err.code == 11000 {
                        return Err(DaoError::DuplicateKey(write_err.message.clone()));
                    }
                }
                Err(DaoError::Mongo(e))
            }
        }
    }

    pub async fn update_one(&self, filter: Document, update: Document) -> DaoResult<bool> {
        let mut set = update
            .get_document("$set")
            .cloned()
            .unwrap_or_default();
        set.insert("updated_at", DateTime::now());

        let mut final_update = update.clone();
        final_update.insert("$set", set);

        let result = self.collection.update_one(filter, final_update).await?;
        Ok(result.modified_count > 0)
    }

    pub async fn update_by_id(&self, id: ObjectId, update: Document) -> DaoResult<bool> {
        self.update_one(doc! { "_id": id }, update).await
    }

    pub async fn soft_delete(&self, id: ObjectId) -> DaoResult<bool> {
        let result = self
            .collection
            .update_one(
                doc! { "_id": id },
                doc! { "$set": { "deleted_at": DateTime::now(), "updated_at": DateTime::now() } },
            )
            .await?;
        Ok(result.modified_count > 0)
    }

    pub async fn hard_delete(&self, filter: Document) -> DaoResult<u64> {
        let result = self.collection.delete_many(filter).await?;
        Ok(result.deleted_count)
    }

    pub async fn count(&self, filter: Document) -> DaoResult<u64> {
        Ok(self.collection.count_documents(filter).await?)
    }
}
