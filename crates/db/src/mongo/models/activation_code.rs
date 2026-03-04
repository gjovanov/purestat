use bson::{oid::ObjectId, DateTime};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivationCode {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub user_id: ObjectId,
    pub token: String,
    pub valid_to: DateTime,
    pub created_at: DateTime,
}

impl ActivationCode {
    pub const COLLECTION: &'static str = "activation_codes";
}
