use bson::{oid::ObjectId, DateTime};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum InviteStatus {
    Pending,
    Accepted,
    Revoked,
    Expired,
}

impl Default for InviteStatus {
    fn default() -> Self {
        InviteStatus::Pending
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invite {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub org_id: ObjectId,
    pub code: String,
    pub inviter_id: ObjectId,
    pub target_email: Option<String>,
    pub role: super::org_member::OrgRole,
    pub max_uses: u32,
    #[serde(default)]
    pub use_count: u32,
    pub expires_at: DateTime,
    #[serde(default)]
    pub status: InviteStatus,
    pub created_at: DateTime,
}

impl Invite {
    pub const COLLECTION: &'static str = "invites";
}
