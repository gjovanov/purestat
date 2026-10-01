//! GHSA-7f5h-5qwr-rxh5: every object addressed by id is resolved WITHIN the
//! caller's org. An admin of org A who puts org B's object id into org A's path
//! gets 404, and org B's object is unchanged afterwards. Anyone can create an
//! org, so membership of one must not reach into another.

use crate::helpers::{user_with_site, TestClient};
use serde_json::{json, Value};

async fn list(client: &TestClient, path: &str) -> Vec<Value> {
    let resp = client.get(path).await;
    assert_eq!(resp.status(), 200, "listing {path}");
    resp.json().await.unwrap()
}

async fn created_id(resp: reqwest::Response) -> String {
    assert_eq!(resp.status(), 201, "create");
    let body: Value = resp.json().await.unwrap();
    body["id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn test_site_update_and_delete_across_orgs_are_not_found() {
    let (victim, victim_org, victim_site, _) = user_with_site("victim").await;
    let (intruder, intruder_org, _, _) = user_with_site("intruder").await;

    let foreign = format!("/api/org/{intruder_org}/site/{victim_site}");
    let resp = intruder.put(&foreign, &json!({ "name": "renamed by intruder" })).await;
    assert_eq!(resp.status(), 404, "update another org's site");
    assert_eq!(intruder.delete(&foreign).await.status(), 404, "delete another org's site");

    let resp = victim.get(&format!("/api/org/{victim_org}/site/{victim_site}")).await;
    assert_eq!(resp.status(), 200, "the victim's site still exists");
    let site: Value = resp.json().await.unwrap();
    assert_eq!(site["name"], "victim site", "and is unchanged");
}

#[tokio::test]
async fn test_goals_across_orgs_are_not_found() {
    let (victim, victim_org, victim_site, _) = user_with_site("victim").await;
    let (intruder, intruder_org, intruder_site, _) = user_with_site("intruder").await;
    let goal = json!({ "goal_type": "custom_event", "name": "Signup", "event_name": "signup" });
    let own = format!("/api/org/{victim_org}/site/{victim_site}/goal");
    let goal_id = created_id(victim.post(&own, &goal).await).await;

    let foreign = format!("/api/org/{intruder_org}/site/{victim_site}/goal");
    assert_eq!(intruder.get(&foreign).await.status(), 404, "list another org's goals");
    assert_eq!(intruder.post(&foreign, &goal).await.status(), 404, "create a goal on another org's site");
    assert_eq!(intruder.delete(&format!("{foreign}/{goal_id}")).await.status(), 404, "delete another org's goal");
    // Through the intruder's OWN site, the goal id still isn't theirs.
    let own_path = format!("/api/org/{intruder_org}/site/{intruder_site}/goal/{goal_id}");
    assert_eq!(intruder.delete(&own_path).await.status(), 404, "delete another site's goal by id");

    assert_eq!(list(&victim, &own).await.len(), 1, "the victim's goal is still there");
}

#[tokio::test]
async fn test_api_keys_across_orgs_are_not_found() {
    let (victim, victim_org, victim_site, _) = user_with_site("victim").await;
    let (intruder, intruder_org, intruder_site, _) = user_with_site("intruder").await;
    let own = format!("/api/org/{victim_org}/site/{victim_site}/api-key");
    let key_id = created_id(victim.post(&own, &json!({ "name": "Victim key" })).await).await;

    let foreign = format!("/api/org/{intruder_org}/site/{victim_site}/api-key");
    assert_eq!(intruder.get(&foreign).await.status(), 404, "list another org's keys");
    let resp = intruder.post(&foreign, &json!({ "name": "Intruder key" })).await;
    assert_eq!(resp.status(), 404, "mint a key on another org's site");
    assert_eq!(intruder.delete(&format!("{foreign}/{key_id}")).await.status(), 404, "revoke another org's key");
    let own_path = format!("/api/org/{intruder_org}/site/{intruder_site}/api-key/{key_id}");
    assert_eq!(intruder.delete(&own_path).await.status(), 404, "revoke another site's key by id");

    let keys = list(&victim, &own).await;
    assert_eq!(keys.len(), 1, "no key was minted on the victim's site");
    assert!(keys[0]["revoked_at"].is_null(), "and the victim's key is not revoked");
}

#[tokio::test]
async fn test_members_across_orgs_are_not_found() {
    let (victim, victim_org, _, _) = user_with_site("victim").await;
    let (intruder, intruder_org, _, _) = user_with_site("intruder").await;
    let members_path = format!("/api/org/{victim_org}/member");
    let owner_member = list(&victim, &members_path).await[0]["id"].as_str().unwrap().to_string();

    let foreign = format!("/api/org/{intruder_org}/member/{owner_member}");
    let resp = intruder.put(&foreign, &json!({ "role": "viewer" })).await;
    assert_eq!(resp.status(), 404, "change a role in another org");
    assert_eq!(intruder.delete(&foreign).await.status(), 404, "remove a member of another org");

    let members = list(&victim, &members_path).await;
    assert_eq!(members.len(), 1, "the victim's membership still exists");
    assert_eq!(members[0]["role"], "owner", "and the victim is still the owner");
}

#[tokio::test]
async fn test_the_owner_role_cannot_be_changed() {
    let (owner, org, _, _) = user_with_site("owner").await;
    let members_path = format!("/api/org/{org}/member");
    let member = list(&owner, &members_path).await[0]["id"].as_str().unwrap().to_string();

    let resp = owner.put(&format!("{members_path}/{member}"), &json!({ "role": "admin" })).await;
    assert_eq!(resp.status(), 403, "the owner's role is not changed");
    assert_eq!(list(&owner, &members_path).await[0]["role"], "owner", "the owner is still the owner");
}

#[tokio::test]
async fn test_invite_revoke_across_orgs_is_not_found() {
    let (victim, victim_org, _, _) = user_with_site("victim").await;
    let (intruder, intruder_org, _, _) = user_with_site("intruder").await;
    let own = format!("/api/org/{victim_org}/invite");
    let invite = json!({ "role": "viewer", "max_uses": 1, "expires_in_hours": 24 });
    let invite_id = created_id(victim.post(&own, &invite).await).await;

    let foreign = format!("/api/org/{intruder_org}/invite/{invite_id}");
    assert_eq!(intruder.delete(&foreign).await.status(), 404, "revoke another org's invite");

    assert_eq!(list(&victim, &own).await.len(), 1, "the victim's invite is still pending");
}
