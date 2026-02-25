use bson::{oid::ObjectId, DateTime};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum OrgRole {
    Owner,
    Admin,
    Viewer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrgMember {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub org_id: ObjectId,
    pub user_id: ObjectId,
    pub role: OrgRole,
    pub joined_at: DateTime,
    pub invited_by: Option<ObjectId>,
}

impl OrgMember {
    pub const COLLECTION: &'static str = "org_members";
}
