//! Sign-in with Google and GitHub (PS6). The routes are in
//! `purestat_api::routes::oauth`; the rules are here.
//!
//! - **State and PKCE** ([`flow`]): every redirect to a provider carries a
//!   fresh `state` and an S256 PKCE challenge. Both are bound to the browser by
//!   a short-lived HttpOnly cookie whose value keys a server-side record. The
//!   callback consumes that record once, compares `state` in constant time,
//!   and sends the PKCE verifier in the code exchange. A missing, mismatched,
//!   expired or reused state is refused before any account work.
//! - **Email trust** ([`trust`]): an address counts only when the provider
//!   vouches for it. Google: `email_verified` is true. GitHub: the primary
//!   address of `GET /user/emails`, if it is verified.
//! - **Accounts** ([`accounts`]): an identity is `(provider, provider_id)`, and
//!   a returning person is found by it first. A vouched address links to an
//!   existing account only when that account proved the address too;
//!   otherwise the sign-in is refused. An account created here is verified,
//!   because the provider vouched for its address.

mod accounts;
mod client;
mod flow;
mod trust;

pub use accounts::{Resolution, resolve};
pub use client::{OAuthClient, ProviderEndpoints, ProviderError};
pub use flow::{
    BegunFlow, FLOW_TTL_SECS, FlowError, FlowFailure, FlowRefusal, FlowStore, OAuthFlows,
    RedisFlowStore, code_challenge,
};
pub use trust::{
    AccountFacts, Decision, GitHubEmail, GoogleUserInfo, ProviderIdentity, Refusal, decide,
    email_proven, github_vouched_email, google_vouched_email,
};

/// A provider purestat signs in with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Provider {
    Google,
    GitHub,
}

impl Provider {
    /// Every provider, in the order the sign-in page shows them.
    pub const ALL: [Provider; 2] = [Provider::Google, Provider::GitHub];

    /// The provider a path segment names. Exact: `Google` names none.
    pub fn from_slug(slug: &str) -> Option<Self> {
        match slug {
            "google" => Some(Self::Google),
            "github" => Some(Self::GitHub),
            _ => None,
        }
    }

    /// The name in paths and in `oauth_providers.provider`.
    pub fn slug(self) -> &'static str {
        match self {
            Self::Google => "google",
            Self::GitHub => "github",
        }
    }

    /// The name people read.
    pub fn name(self) -> &'static str {
        match self {
            Self::Google => "Google",
            Self::GitHub => "GitHub",
        }
    }
}

/// Providers the earlier OAuth code listed but never finished: its callback
/// answered "not implemented" for all three. Their routes answer 404 with
/// this name, so an old link says why instead of failing blankly.
pub fn retired_provider(slug: &str) -> Option<&'static str> {
    match slug {
        "facebook" => Some("Facebook"),
        "linkedin" => Some("LinkedIn"),
        "microsoft" => Some("Microsoft"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_provider_is_named_exactly() {
        assert_eq!(Provider::from_slug("google"), Some(Provider::Google));
        assert_eq!(Provider::from_slug("github"), Some(Provider::GitHub));
        for other in [
            "Google",
            "GITHUB",
            "google ",
            "",
            "githubx",
            "facebook",
            "microsoft",
        ] {
            assert_eq!(Provider::from_slug(other), None, "{other:?}");
        }
        for p in Provider::ALL {
            assert_eq!(Provider::from_slug(p.slug()), Some(p));
        }
    }

    #[test]
    fn the_three_retired_providers_are_named_and_nothing_else_is() {
        assert_eq!(retired_provider("facebook"), Some("Facebook"));
        assert_eq!(retired_provider("linkedin"), Some("LinkedIn"));
        assert_eq!(retired_provider("microsoft"), Some("Microsoft"));
        assert_eq!(retired_provider("google"), None);
        assert_eq!(retired_provider("twitter"), None);
    }
}
