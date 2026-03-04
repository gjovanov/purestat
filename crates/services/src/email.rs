use serde::Serialize;
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub struct EmailService {
    client: reqwest::Client,
    api_key: String,
    from_email: String,
    from_name: String,
}

#[derive(Debug, Serialize)]
struct SendGridRequest {
    personalizations: Vec<Personalization>,
    from: EmailAddress,
    subject: String,
    content: Vec<Content>,
}

#[derive(Debug, Serialize)]
struct Personalization {
    to: Vec<EmailAddress>,
}

#[derive(Debug, Serialize)]
struct EmailAddress {
    email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

#[derive(Debug, Serialize)]
struct Content {
    #[serde(rename = "type")]
    content_type: String,
    value: String,
}

impl EmailService {
    pub fn new(api_key: String, from_email: String, from_name: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
            from_email,
            from_name,
        }
    }

    pub async fn send(
        &self,
        to_email: &str,
        subject: &str,
        html_body: &str,
    ) -> anyhow::Result<()> {
        let request = SendGridRequest {
            personalizations: vec![Personalization {
                to: vec![EmailAddress {
                    email: to_email.to_string(),
                    name: None,
                }],
            }],
            from: EmailAddress {
                email: self.from_email.clone(),
                name: Some(self.from_name.clone()),
            },
            subject: subject.to_string(),
            content: vec![Content {
                content_type: "text/html".to_string(),
                value: html_body.to_string(),
            }],
        };

        let resp = self
            .client
            .post("https://api.sendgrid.com/v3/mail/send")
            .bearer_auth(&self.api_key)
            .json(&request)
            .send()
            .await?;

        if resp.status().is_success() {
            info!(to = to_email, subject, "Email sent via SendGrid");
            Ok(())
        } else {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            warn!(to = to_email, %status, body, "SendGrid email failed");
            anyhow::bail!("SendGrid error {}: {}", status, body)
        }
    }

    pub async fn send_activation(
        &self,
        to_email: &str,
        display_name: &str,
        activation_url: &str,
        ttl_minutes: u64,
    ) -> anyhow::Result<()> {
        let subject = "Activate your PureStat account".to_string();
        let html = format!(
            r#"<div style="font-family: sans-serif; max-width: 600px; margin: 0 auto;">
<h2>Welcome, {name}!</h2>
<p>Please activate your account by clicking the button below. This link expires in {ttl} minutes.</p>
<p style="margin: 32px 0;">
  <a href="{url}" style="background: #4f46e5; color: #fff; padding: 12px 24px; border-radius: 6px; text-decoration: none; font-weight: bold;">
    Activate Account
  </a>
</p>
<p style="color: #666; font-size: 13px;">Or copy this link: <a href="{url}">{url}</a></p>
<p style="color: #999; font-size: 12px; margin-top: 32px;">If you did not create an account, please ignore this email.</p>
</div>"#,
            name = display_name,
            url = activation_url,
            ttl = ttl_minutes,
        );
        self.send(to_email, &subject, &html).await
    }

    pub async fn send_activation_success(
        &self,
        to_email: &str,
        display_name: &str,
        login_url: &str,
    ) -> anyhow::Result<()> {
        let subject = "Your PureStat account is active".to_string();
        let html = format!(
            r#"<div style="font-family: sans-serif; max-width: 600px; margin: 0 auto;">
<h2>Account activated, {name}!</h2>
<p>Your PureStat account is now active. You can <a href="{url}">sign in here</a>.</p>
<p style="color: #999; font-size: 12px; margin-top: 32px;">— The PureStat Team</p>
</div>"#,
            name = display_name,
            url = login_url,
        );
        self.send(to_email, &subject, &html).await
    }
}
