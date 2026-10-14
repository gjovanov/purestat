use bson::{DateTime, oid::ObjectId};
use serde::{Deserialize, Serialize};

/// What a funnel step matches.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FunnelStepType {
    /// A pageview whose path matches `pattern`: an exact path, or one with
    /// `*` wildcards.
    Page,
    /// A completion of the goal `goal_id`, as the goal defines it.
    Goal,
}

/// One step of a funnel. A page step carries `pattern`, a goal step
/// `goal_id`; `label` is an optional name to show instead of either.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FunnelStep {
    #[serde(rename = "type")]
    pub step_type: FunnelStepType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goal_id: Option<ObjectId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// An ordered list of steps a visit can go through, defined for one site.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Funnel {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub site_id: ObjectId,
    pub org_id: ObjectId,
    pub name: String,
    pub steps: Vec<FunnelStep>,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

impl Funnel {
    pub const COLLECTION: &'static str = "funnels";
    /// The fewest and the most steps a funnel has.
    pub const MIN_STEPS: usize = 2;
    pub const MAX_STEPS: usize = 8;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_round_trip_through_bson_with_their_type_named_type() {
        let goal = ObjectId::new();
        let funnel = Funnel {
            id: None,
            site_id: ObjectId::new(),
            org_id: ObjectId::new(),
            name: "Blog to signup".into(),
            steps: vec![
                FunnelStep {
                    step_type: FunnelStepType::Page,
                    pattern: Some("/blog/*".into()),
                    goal_id: None,
                    label: Some("Read a post".into()),
                },
                FunnelStep {
                    step_type: FunnelStepType::Goal,
                    pattern: None,
                    goal_id: Some(goal),
                    label: None,
                },
            ],
            created_at: DateTime::now(),
            updated_at: DateTime::now(),
        };
        let stored = bson::to_document(&funnel).unwrap();
        let steps = stored.get_array("steps").unwrap();
        let first = steps[0].as_document().unwrap();
        assert_eq!(first.get_str("type").unwrap(), "page");
        assert_eq!(first.get_str("pattern").unwrap(), "/blog/*");
        assert!(!first.contains_key("goal_id"), "no empty fields: {first}");
        let second = steps[1].as_document().unwrap();
        assert_eq!(second.get_str("type").unwrap(), "goal");
        assert_eq!(second.get_object_id("goal_id").unwrap(), goal);

        let back: Funnel = bson::from_document(stored).unwrap();
        assert_eq!(back.steps, funnel.steps);
    }
}
