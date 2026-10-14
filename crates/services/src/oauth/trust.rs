//! Which address a provider vouches for, and what a sign-in may do with it.
//!
//! An account is keyed by its address (a unique index on `users.email`), so
//! an address taken on a provider's word decides whose account a person gets.
//! Two rules keep that word honest:
//!
//! 1. Only an address the provider VERIFIED counts. A provider account can
//!    carry an address its holder never proved (Google reports
//!    `email_verified: false`; GitHub lets anyone add any address, unverified,
//!    and its public profile email is free text). Linking on such an address
//!    hands the real owner's purestat account to whoever typed it.
//! 2. A vouched address joins an existing account only if that account proved
//!    the address as well. Otherwise someone could register the victim's
//!    address with a password of their own, never activate it, and collect
//!    the victim's later Google sign-in, with the password still working.
//!
//! Anything else is refused, with a reason that does not depend on what other
//! people's accounts hold: [`Refusal::EmailUnverified`] is decided from the
//! provider's answer alone, and [`Refusal::AccountExists`] is shown only to
//! someone the provider has just confirmed owns the address.

use bson::oid::ObjectId;
use purestat_db::models::User;
use serde::{Deserialize, Deserializer};

use super::Provider;

/// The person signing in, as the provider describes them, after the trust
/// rules have been applied.
#[derive(Debug, Clone)]
pub struct ProviderIdentity {
    pub provider: Provider,
    /// The provider's id for the person: Google's `sub`, GitHub's numeric id.
    pub subject: String,
    /// An address the provider verified belongs to this person. Only this may
    /// create an account or link to one.
    pub vouched_email: Option<String>,
    pub display_name: String,
    /// Where a new account's username starts from.
    pub username_hint: String,
}

/// Google's OpenID Connect userinfo: the fields purestat reads.
#[derive(Debug, Clone, Deserialize)]
pub struct GoogleUserInfo {
    pub sub: String,
    #[serde(default)]
    pub email: Option<String>,
    /// Google tells clients to check this before trusting `email`.
    #[serde(default, deserialize_with = "flag")]
    pub email_verified: Option<bool>,
    #[serde(default)]
    pub name: Option<String>,
}

/// One entry of GitHub's `GET /user/emails`.
#[derive(Debug, Clone, Deserialize)]
pub struct GitHubEmail {
    pub email: String,
    #[serde(default)]
    pub primary: bool,
    #[serde(default)]
    pub verified: bool,
}

/// The address Google vouches for: `email`, only when `email_verified` is
/// true. Absent is not true.
pub fn google_vouched_email(info: &GoogleUserInfo) -> Option<String> {
    if info.email_verified != Some(true) {
        return None;
    }
    info.email.as_deref().and_then(usable_address)
}

/// The address GitHub vouches for: the PRIMARY one, if it is VERIFIED. A
/// verified address that is not primary does not count, nor does a primary
/// one that is not verified, nor the profile's public `email`.
pub fn github_vouched_email(emails: &[GitHubEmail]) -> Option<String> {
    emails
        .iter()
        .find(|e| e.primary && e.verified)
        .and_then(|e| usable_address(&e.email))
}

fn usable_address(raw: &str) -> Option<String> {
    let email = raw.trim();
    let well_formed = email.len() <= 254
        && !email.chars().any(char::is_whitespace)
        && email
            .split_once('@')
            .is_some_and(|(local, domain)| !local.is_empty() && !domain.is_empty());
    well_formed.then(|| email.to_string())
}

/// Whether `user` proved it controls its address (`User::email_proven`).
///
/// An account from before purestat recorded that (`None`) counts as proven
/// when it is activated AND this deployment activates by emailed link
/// (`activation_is_emailed`, an email service is configured). Without an
/// email service, every account was activated at registration, so its
/// activation proves nothing.
pub fn email_proven(user: &User, activation_is_emailed: bool) -> bool {
    if user.deleted_at.is_some() || !user.is_verified {
        return false;
    }
    match user.email_proven {
        Some(proven) => proven,
        None => activation_is_emailed,
    }
}

/// An account holding the address a provider vouched for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccountFacts {
    pub id: ObjectId,
    pub email_proven: bool,
}

/// What a sign-in does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// The identity is linked to this account already: sign in.
    SignIn(ObjectId),
    /// Link the identity to this account, which proved the same address,
    /// and sign in.
    Link(ObjectId),
    /// No account holds the vouched address: create one, verified.
    Create {
        email: String,
    },
    Refuse(Refusal),
}

/// Why a sign-in was refused. Nothing was created or linked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// The provider vouched for no address.
    EmailUnverified,
    /// An account holds the address but has not proven it, or several
    /// accounts hold it in different letter case.
    AccountExists,
}

impl Refusal {
    /// The code the SPA shows a message for (`/login?oauth_error=`).
    pub fn code(self) -> &'static str {
        match self {
            Self::EmailUnverified => "email_unverified",
            Self::AccountExists => "account_exists",
        }
    }
}

/// The sign-in rule.
///
/// - `linked`: the account the identity `(provider, provider_id)` is linked
///   to. A returning person is found by that, never by address, and signs in
///   whatever their address is now.
/// - `vouched_email`: the address the provider vouched for, if any.
/// - `holders`: the live accounts holding that address (the exact one, or
///   those equal to it ignoring case).
pub fn decide(
    linked: Option<ObjectId>,
    vouched_email: Option<&str>,
    holders: &[AccountFacts],
) -> Decision {
    if let Some(id) = linked {
        return Decision::SignIn(id);
    }
    let Some(email) = vouched_email else {
        return Decision::Refuse(Refusal::EmailUnverified);
    };
    match holders {
        [] => Decision::Create {
            email: email.to_string(),
        },
        [only] if only.email_proven => Decision::Link(only.id),
        _ => Decision::Refuse(Refusal::AccountExists),
    }
}

/// A JSON boolean, or the string `"true"`/`"false"` that some OpenID
/// providers send instead. Nothing else is true.
fn flag<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<bool>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Flag {
        Bool(bool),
        Text(String),
    }
    Ok(match Option::<Flag>::deserialize(deserializer)? {
        Some(Flag::Bool(b)) => Some(b),
        Some(Flag::Text(s)) => Some(s == "true"),
        None => None,
    })
}

#[cfg(test)]
mod tests {
    use bson::DateTime;
    use purestat_db::models::OAuthProvider;
    use serde_json::json;

    use super::*;

    const ADDR: &str = "dana@example.com";

    fn google(email_verified: serde_json::Value) -> GoogleUserInfo {
        serde_json::from_value(json!({
            "sub": "fake-google-sub-1",
            "email": ADDR,
            "email_verified": email_verified,
            "name": "Dana",
        }))
        .unwrap()
    }

    fn github(emails: serde_json::Value) -> Vec<GitHubEmail> {
        serde_json::from_value(emails).unwrap()
    }

    fn user(is_verified: bool, email_proven: Option<bool>) -> User {
        let now = DateTime::now();
        User {
            id: Some(ObjectId::new()),
            email: ADDR.to_string(),
            username: "dana".to_string(),
            display_name: "Dana".to_string(),
            avatar: None,
            password_hash: Some("$argon2id$fake".to_string()),
            oauth_providers: Vec::<OAuthProvider>::new(),
            is_verified,
            email_proven,
            locale: "en".to_string(),
            created_at: now,
            updated_at: now,
            deleted_at: None,
        }
    }

    #[test]
    fn google_vouches_only_with_email_verified_true() {
        assert_eq!(
            google_vouched_email(&google(json!(true))),
            Some(ADDR.into())
        );
        assert_eq!(
            google_vouched_email(&google(json!("true"))),
            Some(ADDR.into())
        );
        for not_true in [
            json!(false),
            json!("false"),
            json!(null),
            json!("yes"),
            json!("TRUE"),
        ] {
            assert_eq!(
                google_vouched_email(&google(not_true.clone())),
                None,
                "{not_true}"
            );
        }
        let absent: GoogleUserInfo =
            serde_json::from_value(json!({ "sub": "1", "email": ADDR })).unwrap();
        assert_eq!(google_vouched_email(&absent), None);
        let no_address: GoogleUserInfo =
            serde_json::from_value(json!({ "sub": "1", "email_verified": true })).unwrap();
        assert_eq!(google_vouched_email(&no_address), None);
    }

    #[test]
    fn github_vouches_only_for_the_primary_address_if_verified() {
        // Primary and verified: that one, wherever it sits in the list.
        assert_eq!(
            github_vouched_email(&github(json!([
                { "email": "old@example.com", "primary": false, "verified": true },
                { "email": ADDR, "primary": true, "verified": true },
            ]))),
            Some(ADDR.into())
        );
        // Primary but unverified: nothing, even with a verified secondary.
        assert_eq!(
            github_vouched_email(&github(json!([
                { "email": ADDR, "primary": true, "verified": false },
                { "email": "other@example.com", "primary": false, "verified": true },
            ]))),
            None
        );
        // Verified but not primary: nothing.
        assert_eq!(
            github_vouched_email(&github(json!([
                { "email": ADDR, "primary": false, "verified": true, "visibility": null },
            ]))),
            None
        );
        assert_eq!(github_vouched_email(&[]), None);
    }

    #[test]
    fn an_address_must_look_like_one() {
        assert_eq!(usable_address("  dana@example.com "), Some(ADDR.into()));
        for bad in ["", "dana", "@example.com", "dana@", "da na@example.com"] {
            assert_eq!(usable_address(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn an_account_proved_its_address_by_the_emailed_link_or_a_provider() {
        // Activated by the emailed link, or linked or created by a provider.
        assert!(email_proven(&user(true, Some(true)), false));
        // Registered, never activated.
        assert!(!email_proven(&user(false, Some(false)), true));
        // Activated at registration because no email service is configured.
        assert!(!email_proven(&user(true, Some(false)), true));
        // From before the field: activation proves the address only where
        // activation is by emailed link.
        assert!(email_proven(&user(true, None), true));
        assert!(!email_proven(&user(true, None), false));
        assert!(!email_proven(&user(false, None), true));
        // A deleted account proves nothing.
        let mut deleted = user(true, Some(true));
        deleted.deleted_at = Some(DateTime::now());
        assert!(!email_proven(&deleted, true));
    }

    /// The account holding the sign-in's address, if any, as `decide` sees it.
    #[derive(Clone, Copy, Debug)]
    enum Holder {
        None,
        Proven,
        Unproven,
    }

    #[derive(Debug, PartialEq, Eq)]
    enum Outcome {
        Create,
        Link,
        Refuse(Refusal),
    }

    fn outcome(vouched: Option<String>, holder: Holder) -> (Outcome, ObjectId) {
        let id = ObjectId::new();
        let holders = match holder {
            Holder::None => vec![],
            Holder::Proven => vec![AccountFacts {
                id,
                email_proven: true,
            }],
            Holder::Unproven => vec![AccountFacts {
                id,
                email_proven: false,
            }],
        };
        let decided = match decide(None, vouched.as_deref(), &holders) {
            Decision::Create { email } => {
                assert_eq!(Some(email), vouched);
                Outcome::Create
            }
            Decision::Link(linked) => {
                assert_eq!(linked, id);
                Outcome::Link
            }
            Decision::Refuse(why) => Outcome::Refuse(why),
            Decision::SignIn(_) => panic!("no identity is linked"),
        };
        (decided, id)
    }

    /// The email-trust decision table, provider answer by account state.
    #[test]
    fn the_decision_table() {
        use Holder as H;
        use Outcome::*;
        let google_verified = google_vouched_email(&google(json!(true)));
        let google_unverified = google_vouched_email(&google(json!(false)));
        let github_primary_verified = github_vouched_email(&github(json!([
            { "email": ADDR, "primary": true, "verified": true },
        ])));
        let github_primary_unverified = github_vouched_email(&github(json!([
            { "email": ADDR, "primary": true, "verified": false },
            { "email": "x@example.com", "primary": false, "verified": true },
        ])));
        let github_verified_not_primary = github_vouched_email(&github(json!([
            { "email": "x@example.com", "primary": true, "verified": false },
            { "email": ADDR, "primary": false, "verified": true },
        ])));

        let table = [
            ("google verified", &google_verified, H::None, Create),
            ("google verified", &google_verified, H::Proven, Link),
            (
                "google verified",
                &google_verified,
                H::Unproven,
                Refuse(Refusal::AccountExists),
            ),
            (
                "google unverified",
                &google_unverified,
                H::None,
                Refuse(Refusal::EmailUnverified),
            ),
            (
                "google unverified",
                &google_unverified,
                H::Proven,
                Refuse(Refusal::EmailUnverified),
            ),
            (
                "google unverified",
                &google_unverified,
                H::Unproven,
                Refuse(Refusal::EmailUnverified),
            ),
            (
                "github primary+verified",
                &github_primary_verified,
                H::None,
                Create,
            ),
            (
                "github primary+verified",
                &github_primary_verified,
                H::Proven,
                Link,
            ),
            (
                "github primary+verified",
                &github_primary_verified,
                H::Unproven,
                Refuse(Refusal::AccountExists),
            ),
            (
                "github primary unverified",
                &github_primary_unverified,
                H::None,
                Refuse(Refusal::EmailUnverified),
            ),
            (
                "github primary unverified",
                &github_primary_unverified,
                H::Proven,
                Refuse(Refusal::EmailUnverified),
            ),
            (
                "github primary unverified",
                &github_primary_unverified,
                H::Unproven,
                Refuse(Refusal::EmailUnverified),
            ),
            (
                "github verified not primary",
                &github_verified_not_primary,
                H::None,
                Refuse(Refusal::EmailUnverified),
            ),
            (
                "github verified not primary",
                &github_verified_not_primary,
                H::Proven,
                Refuse(Refusal::EmailUnverified),
            ),
            (
                "github verified not primary",
                &github_verified_not_primary,
                H::Unproven,
                Refuse(Refusal::EmailUnverified),
            ),
        ];
        for (provider, vouched, holder, expected) in table {
            let (got, _) = outcome(vouched.clone(), holder);
            assert_eq!(got, expected, "{provider}, existing account {holder:?}");
        }
    }

    #[test]
    fn a_returning_identity_signs_in_whatever_its_address_is_now() {
        let linked = ObjectId::new();
        let other = AccountFacts {
            id: ObjectId::new(),
            email_proven: false,
        };
        assert_eq!(decide(Some(linked), None, &[]), Decision::SignIn(linked));
        assert_eq!(
            decide(Some(linked), Some(ADDR), &[other]),
            Decision::SignIn(linked)
        );
    }

    #[test]
    fn an_address_held_twice_in_different_case_is_refused() {
        let a = AccountFacts {
            id: ObjectId::new(),
            email_proven: true,
        };
        let b = AccountFacts {
            id: ObjectId::new(),
            email_proven: true,
        };
        assert_eq!(
            decide(None, Some(ADDR), &[a, b]),
            Decision::Refuse(Refusal::AccountExists)
        );
    }

    #[test]
    fn refusal_codes_are_the_ones_the_spa_knows() {
        assert_eq!(Refusal::EmailUnverified.code(), "email_unverified");
        assert_eq!(Refusal::AccountExists.code(), "account_exists");
    }
}
