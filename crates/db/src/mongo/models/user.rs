use bson::{oid::ObjectId, DateTime};
use serde::{Deserialize, Serialize};

/// A sign-in identity at a provider: `provider` is `google` or `github`,
/// `provider_id` the provider's own id for the person (Google's `sub`,
/// GitHub's numeric user id). An identity is found by this pair, never by
/// email.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OAuthProvider {
    pub provider: String,
    pub provider_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub email: String,
    pub username: String,
    pub display_name: String,
    pub avatar: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password_hash: Option<String>,
    #[serde(default)]
    pub oauth_providers: Vec<OAuthProvider>,
    /// The account may sign in with its password (activated). Without an
    /// email service the API sets this at registration, with no proof that
    /// the address is the registrant's; `email_proven` says which.
    #[serde(default)]
    pub is_verified: bool,
    /// Whether this account proved it controls `email`:
    /// `Some(true)` after the emailed activation link, or when Google or
    /// GitHub vouched for the address; `Some(false)` until then, and for an
    /// account activated without any email (no email service configured);
    /// `None` on an account from before this field existed, whose
    /// `is_verified` is all there is to go on.
    ///
    /// OAuth links a sign-in to an existing account by email only when this
    /// is true (`purestat_services::oauth::email_proven`). The proof is of
    /// the address the account holds now: nothing changes `email` today, and
    /// anything that does must set this back to `Some(false)`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email_proven: Option<bool>,
    #[serde(default = "default_locale")]
    pub locale: String,
    pub created_at: DateTime,
    pub updated_at: DateTime,
    pub deleted_at: Option<DateTime>,
}

fn default_locale() -> String {
    "en".to_string()
}

impl User {
    pub const COLLECTION: &'static str = "users";
}
