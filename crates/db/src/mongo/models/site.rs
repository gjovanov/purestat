use bson::{oid::ObjectId, DateTime};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Site {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub org_id: ObjectId,
    pub domain: String,
    pub name: String,
    #[serde(default = "default_timezone")]
    pub timezone: String,
    #[serde(default)]
    pub is_public: bool,
    #[serde(default)]
    pub allowed_hostnames: Vec<String>,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

fn default_timezone() -> String {
    "UTC".to_string()
}

impl Site {
    pub const COLLECTION: &'static str = "sites";
}
