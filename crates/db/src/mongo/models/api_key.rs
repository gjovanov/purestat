use bson::{oid::ObjectId, DateTime};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub site_id: ObjectId,
    pub org_id: ObjectId,
    pub name: String,
    pub key_hash: String,
    pub key_prefix: String,
    #[serde(default)]
    pub scopes: Vec<String>,
    pub last_used_at: Option<DateTime>,
    pub created_at: DateTime,
    pub revoked_at: Option<DateTime>,
}

impl ApiKey {
    pub const COLLECTION: &'static str = "api_keys";
}
