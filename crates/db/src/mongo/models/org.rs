use bson::{oid::ObjectId, DateTime};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Plan {
    Free,
    Pro,
    Business,
}

impl Default for Plan {
    fn default() -> Self {
        Plan::Free
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillingInfo {
    pub customer_id: String,
    pub subscription_id: Option<String>,
    pub period_end: Option<DateTime>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanLimits {
    pub max_sites: u32,
    pub max_members: u32,
    pub max_pageviews_monthly: u64,
}

impl Default for PlanLimits {
    fn default() -> Self {
        PlanLimits::for_plan(&Plan::Free)
    }
}

impl PlanLimits {
    pub fn for_plan(plan: &Plan) -> Self {
        match plan {
            Plan::Free => PlanLimits {
                max_sites: 1,
                max_members: 1,
                max_pageviews_monthly: 10_000,
            },
            Plan::Pro => PlanLimits {
                max_sites: 5,
                max_members: 5,
                max_pageviews_monthly: 100_000,
            },
            Plan::Business => PlanLimits {
                max_sites: u32::MAX,
                max_members: u32::MAX,
                max_pageviews_monthly: 1_000_000,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub current_month_pageviews: u64,
    pub usage_reset_at: DateTime,
}

impl Default for Usage {
    fn default() -> Self {
        Usage {
            current_month_pageviews: 0,
            usage_reset_at: DateTime::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Org {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub name: String,
    pub slug: String,
    pub owner_id: ObjectId,
    #[serde(default)]
    pub plan: Plan,
    pub billing: Option<BillingInfo>,
    #[serde(default)]
    pub limits: PlanLimits,
    #[serde(default)]
    pub usage: Usage,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

impl Org {
    pub const COLLECTION: &'static str = "orgs";
}
