use axum::extract::State;
use axum::http::HeaderMap;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::error::ApiError;
use crate::extractors::auth::AuthUser;
use crate::routes::org::parse_oid;
use crate::state::AppState;

#[derive(Serialize)]
pub struct PlansResponse {
    pub plans: Vec<PlanInfo>,
}

#[derive(Serialize)]
pub struct PlanInfo {
    pub name: String,
    pub price: String,
    pub pageviews: String,
    pub sites: String,
    pub members: String,
}

#[derive(Deserialize)]
pub struct CheckoutRequest {
    pub org_id: String,
    pub plan: String,
    pub success_url: String,
    pub cancel_url: String,
}

#[derive(Deserialize)]
pub struct PortalRequest {
    pub org_id: String,
    pub return_url: String,
}

pub async fn plans() -> Json<PlansResponse> {
    Json(PlansResponse {
        plans: vec![
            PlanInfo {
                name: "Free".to_string(),
                price: "$0/mo".to_string(),
                pageviews: "10,000".to_string(),
                sites: "1".to_string(),
                members: "1".to_string(),
            },
            PlanInfo {
                name: "Pro".to_string(),
                price: "$9/mo".to_string(),
                pageviews: "100,000".to_string(),
                sites: "5".to_string(),
                members: "5".to_string(),
            },
            PlanInfo {
                name: "Business".to_string(),
                price: "$29/mo".to_string(),
                pageviews: "1,000,000".to_string(),
                sites: "Unlimited".to_string(),
                members: "Unlimited".to_string(),
            },
        ],
    })
}

pub async fn checkout(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<CheckoutRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let org_oid = parse_oid(&body.org_id)?;
    let org = state.orgs.base.find_by_id(org_oid).await?;

    if org.owner_id != auth.user_id {
        return Err(ApiError::Forbidden(
            "Only the owner can manage billing".to_string(),
        ));
    }

    let price_id = match body.plan.as_str() {
        "pro" => state.stripe.pro_price_id().to_string(),
        "business" => state.stripe.business_price_id().to_string(),
        _ => {
            return Err(ApiError::BadRequest(format!(
                "Unknown plan: {}",
                body.plan
            )))
        }
    };

    let session = state
        .stripe
        .create_checkout_session(
            &price_id,
            &auth.email,
            &body.org_id,
            &body.success_url,
            &body.cancel_url,
        )
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(serde_json::json!({
        "url": session.url,
        "session_id": session.session_id
    })))
}

pub async fn portal(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<PortalRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let org_oid = parse_oid(&body.org_id)?;
    let org = state.orgs.base.find_by_id(org_oid).await?;

    if org.owner_id != auth.user_id {
        return Err(ApiError::Forbidden(
            "Only the owner can manage billing".to_string(),
        ));
    }

    let customer_id = org
        .billing
        .as_ref()
        .map(|b| b.customer_id.clone())
        .ok_or_else(|| {
            ApiError::BadRequest("No billing information found".to_string())
        })?;

    let portal = state
        .stripe
        .create_portal_session(&customer_id, &body.return_url)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(serde_json::json!({ "url": portal.url })))
}

pub async fn webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> Result<Json<serde_json::Value>, ApiError> {
    let signature = headers
        .get("stripe-signature")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            ApiError::BadRequest("Missing Stripe signature".to_string())
        })?;

    state
        .stripe
        .verify_webhook_signature(&body, signature)
        .map_err(|_| ApiError::BadRequest("Invalid webhook signature".to_string()))?;

    let event: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;

    let event_type = event["type"].as_str().unwrap_or("");

    match event_type {
        "checkout.session.completed" => {
            let org_id_str = event["data"]["object"]["metadata"]["org_id"]
                .as_str()
                .unwrap_or("");
            let customer_id = event["data"]["object"]["customer"]
                .as_str()
                .unwrap_or("");
            let subscription_id = event["data"]["object"]["subscription"]
                .as_str()
                .unwrap_or("");

            if let Ok(org_oid) = bson::oid::ObjectId::parse_str(org_id_str) {
                let billing = purestat_db::models::BillingInfo {
                    customer_id: customer_id.to_string(),
                    subscription_id: Some(subscription_id.to_string()),
                    period_end: None,
                };
                let _ = state.orgs.base.update_by_id(
                    org_oid,
                    bson::doc! { "$set": { "billing": bson::to_bson(&billing).unwrap_or_default() } },
                ).await;
            }
        }
        "customer.subscription.updated" | "customer.subscription.deleted" => {
            tracing::info!(event_type = event_type, "Stripe subscription event");
        }
        _ => {
            tracing::debug!(event_type = event_type, "Unhandled Stripe event");
        }
    }

    Ok(Json(serde_json::json!({ "received": true })))
}
