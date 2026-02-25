use bson::{oid::ObjectId, DateTime};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum GoalType {
    Pageview,
    CustomEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Goal {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub site_id: ObjectId,
    pub org_id: ObjectId,
    pub goal_type: GoalType,
    pub name: String,
    pub event_name: Option<String>,
    pub page_path: Option<String>,
    pub created_at: DateTime,
}

impl Goal {
    pub const COLLECTION: &'static str = "goals";
}
