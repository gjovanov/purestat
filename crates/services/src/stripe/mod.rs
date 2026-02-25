use purestat_config::StripeSettings;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StripeError {
    #[error("Stripe API error: {0}")]
    Api(String),
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Invalid webhook signature")]
    InvalidSignature,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckoutSession {
    pub url: String,
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortalSession {
    pub url: String,
}

pub struct StripeService {
    settings: StripeSettings,
    client: Client,
}

impl StripeService {
    pub fn new(settings: StripeSettings) -> Self {
        Self {
            settings,
            client: Client::new(),
        }
    }

    pub async fn create_checkout_session(
        &self,
        price_id: &str,
        customer_email: &str,
        org_id: &str,
        success_url: &str,
        cancel_url: &str,
    ) -> Result<CheckoutSession, StripeError> {
        let params = [
            ("mode", "subscription"),
            ("customer_email", customer_email),
            ("success_url", success_url),
            ("cancel_url", cancel_url),
            ("line_items[0][price]", price_id),
            ("line_items[0][quantity]", "1"),
            ("metadata[org_id]", org_id),
        ];

        let resp = self
            .client
            .post("https://api.stripe.com/v1/checkout/sessions")
            .basic_auth(&self.settings.secret_key, None::<&str>)
            .form(&params)
            .send()
            .await?;

        let body: serde_json::Value = resp.json().await?;

        Ok(CheckoutSession {
            url: body["url"].as_str().unwrap_or("").to_string(),
            session_id: body["id"].as_str().unwrap_or("").to_string(),
        })
    }

    pub async fn create_portal_session(
        &self,
        customer_id: &str,
        return_url: &str,
    ) -> Result<PortalSession, StripeError> {
        let params = [
            ("customer", customer_id),
            ("return_url", return_url),
        ];

        let resp = self
            .client
            .post("https://api.stripe.com/v1/billing_portal/sessions")
            .basic_auth(&self.settings.secret_key, None::<&str>)
            .form(&params)
            .send()
            .await?;

        let body: serde_json::Value = resp.json().await?;

        Ok(PortalSession {
            url: body["url"].as_str().unwrap_or("").to_string(),
        })
    }

    pub fn verify_webhook_signature(
        &self,
        payload: &str,
        signature: &str,
    ) -> Result<(), StripeError> {
        // Stripe webhook signature verification
        let parts: Vec<&str> = signature.split(',').collect();
        let mut timestamp = "";
        let mut sig = "";

        for part in &parts {
            if let Some(t) = part.strip_prefix("t=") {
                timestamp = t;
            } else if let Some(s) = part.strip_prefix("v1=") {
                sig = s;
            }
        }

        if timestamp.is_empty() || sig.is_empty() {
            return Err(StripeError::InvalidSignature);
        }

        let signed_payload = format!("{timestamp}.{payload}");
        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        type HmacSha256 = Hmac<Sha256>;

        let mut mac = HmacSha256::new_from_slice(self.settings.webhook_secret.as_bytes())
            .map_err(|_| StripeError::InvalidSignature)?;
        mac.update(signed_payload.as_bytes());
        let expected = hex::encode(mac.finalize().into_bytes());

        if expected != sig {
            return Err(StripeError::InvalidSignature);
        }

        Ok(())
    }

    pub fn pro_price_id(&self) -> &str {
        &self.settings.pro_price_id
    }

    pub fn business_price_id(&self) -> &str {
        &self.settings.business_price_id
    }
}
