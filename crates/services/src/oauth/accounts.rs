//! What a sign-in does to accounts: the rule in [`decide`], applied.

use purestat_db::models::{OAuthProvider, User};

use super::trust::{AccountFacts, Decision, ProviderIdentity, Refusal, decide, email_proven};
use crate::dao::{DaoError, UserDao};

/// How a sign-in ended.
#[derive(Debug)]
pub enum Resolution {
    /// A returning identity: its linked account.
    SignedIn(User),
    /// The identity is now linked to the existing account that proved the
    /// same address.
    Linked(User),
    /// A new account, verified: the provider vouched for its address.
    Created(User),
    /// Nothing was created or linked.
    Refused(Refusal),
}

impl Resolution {
    /// One word for the log.
    pub fn outcome(&self) -> &'static str {
        match self {
            Self::SignedIn(_) => "signed_in",
            Self::Linked(_) => "linked",
            Self::Created(_) => "created",
            Self::Refused(r) => r.code(),
        }
    }
}

/// How many usernames a new account tries before giving up.
const USERNAME_ATTEMPTS: usize = 5;

/// Resolve `identity` to an account.
///
/// `activation_is_emailed`: this deployment activates accounts by emailed
/// link (an email service is configured), so an activated account from
/// before purestat recorded `email_proven` did prove its address.
pub async fn resolve(
    users: &UserDao,
    identity: &ProviderIdentity,
    activation_is_emailed: bool,
) -> Result<Resolution, DaoError> {
    let key = OAuthProvider {
        provider: identity.provider.slug().to_string(),
        provider_id: identity.subject.clone(),
    };
    // A returning person is found by the identity, never by address.
    let linked = users.find_by_oauth(&key.provider, &key.provider_id).await?;
    let holders = match (&linked, identity.vouched_email.as_deref()) {
        (None, Some(email)) => holders_of(users, email).await?,
        _ => Vec::new(),
    };
    let facts: Vec<AccountFacts> = holders
        .iter()
        .filter_map(|u| {
            u.id.map(|id| AccountFacts {
                id,
                email_proven: email_proven(u, activation_is_emailed),
            })
        })
        .collect();

    match decide(
        linked.as_ref().and_then(|u| u.id),
        identity.vouched_email.as_deref(),
        &facts,
    ) {
        Decision::SignIn(_) => Ok(Resolution::SignedIn(linked.ok_or(DaoError::NotFound)?)),
        Decision::Link(id) => {
            if !users.link_oauth(id, &key).await? {
                // Deleted since it was read.
                return Err(DaoError::NotFound);
            }
            Ok(Resolution::Linked(users.base.find_by_id(id).await?))
        }
        Decision::Create { email } => create(users, identity, email, key).await,
        Decision::Refuse(why) => Ok(Resolution::Refused(why)),
    }
}

/// The live accounts holding `email`: the one with exactly this address, or
/// else up to two equal to it ignoring case. Two means the address is
/// ambiguous, which the rule refuses.
async fn holders_of(users: &UserDao, email: &str) -> Result<Vec<User>, DaoError> {
    match users.find_by_email(email).await {
        Ok(user) => Ok(vec![user]),
        Err(DaoError::NotFound) => users.find_by_email_ignoring_case(email, 2).await,
        Err(e) => Err(e),
    }
}

async fn create(
    users: &UserDao,
    identity: &ProviderIdentity,
    email: String,
    key: OAuthProvider,
) -> Result<Resolution, DaoError> {
    let base = username_base(&identity.username_hint);
    for attempt in 0..USERNAME_ATTEMPTS {
        let username = if attempt == 0 {
            base.clone()
        } else {
            format!("{base}-{}", username_suffix())
        };
        match users
            .create_oauth(
                email.clone(),
                username,
                identity.display_name.clone(),
                key.clone(),
            )
            .await
        {
            Ok(user) => return Ok(Resolution::Created(user)),
            Err(DaoError::DuplicateKey(msg)) if duplicate_on(&msg, "username") => continue,
            // The address was taken since it was looked up, or a deleted
            // account still holds it: no account is created.
            Err(DaoError::DuplicateKey(msg)) if duplicate_on(&msg, "email") => {
                return Ok(Resolution::Refused(Refusal::AccountExists));
            }
            Err(e) => return Err(e),
        }
    }
    Err(DaoError::Validation(
        "no free username for a new account".to_string(),
    ))
}

/// Whether MongoDB's duplicate-key message names the unique index on `field`.
fn duplicate_on(message: &str, field: &str) -> bool {
    message.contains(&format!("index: {field}_1 ")) || message.contains(&format!("{{ {field}: "))
}

/// A username from the provider's login or address: lowercase ASCII letters,
/// digits, `.`, `-` and `_`, at most 30 characters, at least 3.
fn username_base(hint: &str) -> String {
    let kept: String = hint
        .chars()
        .map(|c| c.to_ascii_lowercase())
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
        .take(30)
        .collect();
    let trimmed = kept.trim_matches(|c| matches!(c, '.' | '-' | '_'));
    if trimmed.len() >= 3 {
        trimmed.to_string()
    } else {
        format!("user{trimmed}")
    }
}

fn username_suffix() -> String {
    const ALPHABET: [char; 36] = [
        'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r',
        's', 't', 'u', 'v', 'w', 'x', 'y', 'z', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9',
    ];
    nanoid::nanoid!(6, &ALPHABET)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_username_comes_from_the_login_or_address() {
        assert_eq!(username_base("Octo-Cat"), "octo-cat");
        assert_eq!(username_base("dana.smith"), "dana.smith");
        assert_eq!(username_base("j\u{f6}rg+news"), "jrgnews");
        assert_eq!(username_base("--x--"), "userx");
        assert_eq!(username_base(""), "user");
        assert_eq!(username_base(&"a".repeat(80)).len(), 30);
    }

    #[test]
    fn a_suffix_is_six_lowercase_letters_or_digits() {
        let s = username_suffix();
        assert_eq!(s.len(), 6);
        assert!(
            s.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()),
            "{s}"
        );
    }

    #[test]
    fn duplicate_key_messages_name_their_index() {
        let username = r#"E11000 duplicate key error collection: purestat.users index: username_1 dup key: { username: "dana" }"#;
        let email = r#"E11000 duplicate key error collection: purestat.users index: email_1 dup key: { email: "dana@example.com" }"#;
        assert!(duplicate_on(username, "username"));
        assert!(!duplicate_on(username, "email"));
        assert!(duplicate_on(email, "email"));
        assert!(!duplicate_on(email, "username"));
    }

    #[test]
    fn outcomes_name_themselves_for_the_log() {
        assert_eq!(
            Resolution::Refused(Refusal::EmailUnverified).outcome(),
            "email_unverified"
        );
    }
}
