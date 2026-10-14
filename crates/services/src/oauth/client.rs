//! The HTTP half of a sign-in: the address the browser is sent to, the code
//! exchange (with the PKCE verifier), and who signed in.

use std::time::Duration;

use purestat_config::{OAuthProviderSettings, OAuthSettings};
use reqwest::header::ACCEPT;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use url::Url;

use super::Provider;
use super::trust::{
    GitHubEmail, GoogleUserInfo, ProviderIdentity, github_vouched_email, google_vouched_email,
};

/// Google: `openid` for the account id (`sub`), `email` for the address and
/// whether Google verified it, `profile` for the name.
const GOOGLE_SCOPE: &str = "openid email profile";

/// GitHub: read the account's addresses (`GET /user/emails`). The public
/// profile (`GET /user`) needs no scope.
const GITHUB_SCOPE: &str = "user:email";

/// Where a provider's endpoints are. In production they are fixed
/// ([`ProviderEndpoints::production`]); there is no setting for them. A test
/// points them at a mock with [`OAuthClient::with_endpoints`].
#[derive(Debug, Clone)]
pub struct ProviderEndpoints {
    /// Where the browser goes to sign in.
    pub authorize: String,
    /// The code exchange.
    pub token: String,
    /// Who signed in: Google's OpenID userinfo, GitHub's `GET /user`.
    pub profile: String,
    /// GitHub's `GET /user/emails`. Unused for Google.
    pub emails: String,
}

impl ProviderEndpoints {
    /// The provider's real endpoints.
    pub fn production(provider: Provider) -> Self {
        match provider {
            Provider::Google => Self {
                authorize: "https://accounts.google.com/o/oauth2/v2/auth".to_string(),
                token: "https://oauth2.googleapis.com/token".to_string(),
                profile: "https://openidconnect.googleapis.com/v1/userinfo".to_string(),
                emails: String::new(),
            },
            Provider::GitHub => Self {
                authorize: "https://github.com/login/oauth/authorize".to_string(),
                token: "https://github.com/login/oauth/access_token".to_string(),
                profile: "https://api.github.com/user".to_string(),
                emails: "https://api.github.com/user/emails".to_string(),
            },
        }
    }
}

/// A provider's answer that ends the sign-in. Messages carry no code, token
/// or secret.
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("sign-in with {0} is not configured")]
    NotConfigured(&'static str),
    #[error("{provider} {step}: {source}")]
    Http {
        provider: &'static str,
        step: &'static str,
        source: reqwest::Error,
    },
    #[error("{provider} {step}: HTTP {status}")]
    Status {
        provider: &'static str,
        step: &'static str,
        status: u16,
    },
    #[error("{provider} refused the code exchange: {reason}")]
    Exchange {
        provider: &'static str,
        reason: String,
    },
    #[error("{provider} {step}: {detail}")]
    Invalid {
        provider: &'static str,
        step: &'static str,
        detail: String,
    },
}

/// A provider's client registration, and where to reach it.
struct Registration {
    client_id: String,
    client_secret: String,
    endpoints: ProviderEndpoints,
}

#[derive(Deserialize)]
struct TokenResponse {
    #[serde(default)]
    access_token: Option<String>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    error_description: Option<String>,
}

#[derive(Deserialize)]
struct GitHubUser {
    id: u64,
    login: String,
    #[serde(default)]
    name: Option<String>,
}

/// The providers this server signs in with, and the HTTP to them.
pub struct OAuthClient {
    http: reqwest::Client,
    /// `app.frontend_url` without a trailing slash: where the SPA and the API
    /// are reached, so the base of every callback and redirect address.
    public_url: String,
    enabled: bool,
    google: Option<Registration>,
    github: Option<Registration>,
}

impl OAuthClient {
    /// From PURESTAT__OAUTH__* and the public address (`app.frontend_url`).
    /// A provider takes part only with both a client id and a secret.
    pub fn from_settings(settings: &OAuthSettings, public_url: &str) -> Self {
        let register =
            |provider: Provider, configured: &Option<OAuthProviderSettings>| match configured {
                Some(c) if c.is_complete() => Some(Registration {
                    client_id: c.client_id.trim().to_string(),
                    client_secret: c.client_secret.trim().to_string(),
                    endpoints: ProviderEndpoints::production(provider),
                }),
                Some(_) => {
                    if settings.enabled {
                        tracing::warn!(
                            provider = provider.slug(),
                            "OAuth: {} has a client id or a client secret but not both, so \
                             sign-in with it is off",
                            provider.name()
                        );
                    }
                    None
                }
                None => None,
            };
        let client = Self {
            http: reqwest::Client::builder()
                .user_agent("purestat")
                .connect_timeout(Duration::from_secs(5))
                .timeout(Duration::from_secs(10))
                // A provider endpoint never redirects; a token is sent where
                // it was meant to go or not at all.
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .expect("an HTTP client for OAuth"),
            public_url: public_url.trim().trim_end_matches('/').to_string(),
            enabled: settings.enabled,
            google: register(Provider::Google, &settings.google),
            github: register(Provider::GitHub, &settings.github),
        };
        if client.enabled {
            let providers: Vec<&str> = client.providers().into_iter().map(Provider::slug).collect();
            if providers.is_empty() {
                tracing::warn!(
                    "PURESTAT__OAUTH__ENABLED is true, but no provider has both a client id and \
                     a client secret: no sign-in buttons will show"
                );
            } else {
                tracing::info!(
                    providers = ?providers,
                    callback_base = %format!("{}/api/oauth/", client.public_url),
                    "OAuth sign-in enabled"
                );
            }
        }
        client
    }

    /// Point `provider` at other endpoints: a test's mock. No setting reaches
    /// this.
    pub fn with_endpoints(mut self, provider: Provider, endpoints: ProviderEndpoints) -> Self {
        let registration = match provider {
            Provider::Google => self.google.as_mut(),
            Provider::GitHub => self.github.as_mut(),
        };
        if let Some(r) = registration {
            r.endpoints = endpoints;
        }
        self
    }

    fn registration(&self, provider: Provider) -> Result<&Registration, ProviderError> {
        let registration = match provider {
            Provider::Google => self.google.as_ref(),
            Provider::GitHub => self.github.as_ref(),
        };
        registration
            .filter(|_| self.enabled)
            .ok_or(ProviderError::NotConfigured(provider.name()))
    }

    /// Whether a person can sign in with `provider` here: OAuth is enabled
    /// and the provider has a client id and secret.
    pub fn is_available(&self, provider: Provider) -> bool {
        self.registration(provider).is_ok()
    }

    /// The providers a person can sign in with here, in display order.
    pub fn providers(&self) -> Vec<Provider> {
        Provider::ALL
            .into_iter()
            .filter(|p| self.is_available(*p))
            .collect()
    }

    /// The SPA's address, the base of every redirect after a callback.
    pub fn public_url(&self) -> &str {
        &self.public_url
    }

    /// The callback address, as registered at the provider.
    pub fn redirect_uri(&self, provider: Provider) -> String {
        format!("{}/api/oauth/{}/callback", self.public_url, provider.slug())
    }

    /// Where to send the browser: the provider's sign-in page, carrying the
    /// flow's `state` and PKCE challenge.
    pub fn authorize_url(
        &self,
        provider: Provider,
        state: &str,
        code_challenge: &str,
    ) -> Result<String, ProviderError> {
        let registration = self.registration(provider)?;
        let mut url =
            Url::parse(&registration.endpoints.authorize).map_err(|e| ProviderError::Invalid {
                provider: provider.name(),
                step: "authorize address",
                detail: e.to_string(),
            })?;
        {
            let mut query = url.query_pairs_mut();
            query
                .append_pair("client_id", &registration.client_id)
                .append_pair("redirect_uri", &self.redirect_uri(provider))
                .append_pair("state", state)
                .append_pair("code_challenge", code_challenge)
                .append_pair("code_challenge_method", "S256");
            match provider {
                Provider::Google => {
                    query
                        .append_pair("response_type", "code")
                        .append_pair("scope", GOOGLE_SCOPE)
                        // Let the person choose the account rather than
                        // silently use whichever is signed in.
                        .append_pair("prompt", "select_account");
                }
                Provider::GitHub => {
                    query.append_pair("scope", GITHUB_SCOPE);
                }
            }
        }
        Ok(url.into())
    }

    /// Exchange the callback's `code`, with the flow's PKCE `verifier`, for an
    /// access token. The token is used for the identity calls and dropped.
    pub async fn exchange_code(
        &self,
        provider: Provider,
        code: &str,
        verifier: &str,
    ) -> Result<String, ProviderError> {
        let registration = self.registration(provider)?;
        let redirect_uri = self.redirect_uri(provider);
        let mut form: Vec<(&str, &str)> = vec![
            ("client_id", &registration.client_id),
            ("client_secret", &registration.client_secret),
            ("code", code),
            ("redirect_uri", &redirect_uri),
            ("code_verifier", verifier),
        ];
        if provider == Provider::Google {
            form.push(("grant_type", "authorization_code"));
        }
        let step = "code exchange";
        let response = self
            .http
            .post(&registration.endpoints.token)
            .header(ACCEPT, "application/json")
            .form(&form)
            .send()
            .await
            .map_err(|source| ProviderError::Http {
                provider: provider.name(),
                step,
                source,
            })?;
        let status = response.status();
        // Google answers a failed exchange with 400 and JSON, GitHub with 200
        // and JSON, so the body is read either way.
        let TokenResponse {
            access_token,
            error,
            error_description,
        } = response
            .json::<TokenResponse>()
            .await
            .map_err(|_| ProviderError::Status {
                provider: provider.name(),
                step,
                status: status.as_u16(),
            })?;
        match access_token {
            Some(token) if status.is_success() && !token.is_empty() => Ok(token),
            _ => Err(ProviderError::Exchange {
                provider: provider.name(),
                reason: match (error, error_description) {
                    (Some(e), Some(d)) => format!("{e}: {d}"),
                    (Some(e), None) => e,
                    _ => format!("HTTP {status} without an access token"),
                },
            }),
        }
    }

    /// Who signed in: the provider's id for them, and the address the
    /// provider vouches for (see `trust`).
    pub async fn identity(
        &self,
        provider: Provider,
        access_token: &str,
    ) -> Result<ProviderIdentity, ProviderError> {
        let endpoints = &self.registration(provider)?.endpoints;
        match provider {
            Provider::Google => {
                let info: GoogleUserInfo = self
                    .get_json(provider, "userinfo", &endpoints.profile, access_token)
                    .await?;
                if info.sub.trim().is_empty() {
                    return Err(ProviderError::Invalid {
                        provider: provider.name(),
                        step: "userinfo",
                        detail: "no account id (sub)".to_string(),
                    });
                }
                let vouched_email = google_vouched_email(&info);
                let local_part = info
                    .email
                    .as_deref()
                    .and_then(|e| e.split('@').next())
                    .unwrap_or_default()
                    .to_string();
                let display_name =
                    first_non_empty(&[info.name.as_deref(), Some(local_part.as_str())])
                        .unwrap_or(provider.name())
                        .to_string();
                Ok(ProviderIdentity {
                    provider,
                    subject: info.sub,
                    vouched_email,
                    display_name,
                    username_hint: local_part,
                })
            }
            Provider::GitHub => {
                let user: GitHubUser = self
                    .get_json(provider, "user", &endpoints.profile, access_token)
                    .await?;
                if user.id == 0 {
                    return Err(ProviderError::Invalid {
                        provider: provider.name(),
                        step: "user",
                        detail: "no account id".to_string(),
                    });
                }
                let emails: Vec<GitHubEmail> = self
                    .get_json(provider, "user emails", &endpoints.emails, access_token)
                    .await?;
                let display_name =
                    first_non_empty(&[user.name.as_deref(), Some(user.login.as_str())])
                        .unwrap_or(provider.name())
                        .to_string();
                Ok(ProviderIdentity {
                    provider,
                    subject: user.id.to_string(),
                    vouched_email: github_vouched_email(&emails),
                    display_name,
                    username_hint: user.login,
                })
            }
        }
    }

    async fn get_json<T: DeserializeOwned>(
        &self,
        provider: Provider,
        step: &'static str,
        url: &str,
        access_token: &str,
    ) -> Result<T, ProviderError> {
        let request = self.http.get(url).bearer_auth(access_token);
        let request = match provider {
            Provider::Google => request.header(ACCEPT, "application/json"),
            Provider::GitHub => request
                .header(ACCEPT, "application/vnd.github+json")
                .header("X-GitHub-Api-Version", "2022-11-28"),
        };
        let response = request.send().await.map_err(|source| ProviderError::Http {
            provider: provider.name(),
            step,
            source,
        })?;
        let status = response.status();
        if !status.is_success() {
            return Err(ProviderError::Status {
                provider: provider.name(),
                step,
                status: status.as_u16(),
            });
        }
        response
            .json::<T>()
            .await
            .map_err(|e| ProviderError::Invalid {
                provider: provider.name(),
                step,
                detail: e.to_string(),
            })
    }
}

fn first_non_empty<'a>(candidates: &[Option<&'a str>]) -> Option<&'a str> {
    candidates
        .iter()
        .flatten()
        .map(|s| s.trim())
        .find(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn provider_settings(id: &str, secret: &str) -> Option<OAuthProviderSettings> {
        Some(OAuthProviderSettings {
            client_id: id.to_string(),
            client_secret: secret.to_string(),
        })
    }

    fn client(enabled: bool, public_url: &str) -> OAuthClient {
        OAuthClient::from_settings(
            &OAuthSettings {
                enabled,
                google: provider_settings("fake-google-client", "fake-google-secret"),
                github: provider_settings("fake-github-client", "fake-github-secret"),
            },
            public_url,
        )
    }

    fn query(url: &str) -> HashMap<String, String> {
        Url::parse(url)
            .unwrap()
            .query_pairs()
            .into_owned()
            .collect()
    }

    #[test]
    fn only_enabled_and_fully_configured_providers_are_offered() {
        assert!(
            client(false, "https://purestat.example")
                .providers()
                .is_empty()
        );
        assert_eq!(
            client(true, "https://purestat.example").providers(),
            vec![Provider::Google, Provider::GitHub]
        );
        let half = OAuthClient::from_settings(
            &OAuthSettings {
                enabled: true,
                google: provider_settings("fake-google-client", ""),
                github: provider_settings("fake-github-client", "fake-github-secret"),
            },
            "https://purestat.example",
        );
        assert_eq!(half.providers(), vec![Provider::GitHub]);
        assert!(!half.is_available(Provider::Google));
        assert!(matches!(
            half.authorize_url(Provider::Google, "s", "c"),
            Err(ProviderError::NotConfigured("Google"))
        ));
    }

    #[test]
    fn the_callback_address_derives_from_the_public_url() {
        let c = client(true, "https://purestat.example/");
        assert_eq!(
            c.redirect_uri(Provider::Google),
            "https://purestat.example/api/oauth/google/callback"
        );
        assert_eq!(
            c.redirect_uri(Provider::GitHub),
            "https://purestat.example/api/oauth/github/callback"
        );
        assert_eq!(
            client(true, "http://localhost:5173").redirect_uri(Provider::Google),
            "http://localhost:5173/api/oauth/google/callback"
        );
    }

    #[test]
    fn the_google_redirect_carries_state_and_an_s256_challenge() {
        let c = client(true, "https://purestat.example");
        let url = c
            .authorize_url(Provider::Google, "the-state", "the-challenge")
            .unwrap();
        assert!(
            url.starts_with("https://accounts.google.com/o/oauth2/v2/auth?"),
            "{url}"
        );
        let q = query(&url);
        assert_eq!(q["client_id"], "fake-google-client");
        assert_eq!(
            q["redirect_uri"],
            "https://purestat.example/api/oauth/google/callback"
        );
        assert_eq!(q["response_type"], "code");
        assert_eq!(q["scope"], "openid email profile");
        assert_eq!(q["state"], "the-state");
        assert_eq!(q["code_challenge"], "the-challenge");
        assert_eq!(q["code_challenge_method"], "S256");
        // No refresh token is asked for: purestat calls Google once, at sign-in.
        assert!(!q.contains_key("access_type"));
        // The secret never travels through the browser.
        assert!(!url.contains("fake-google-secret"));
    }

    #[test]
    fn the_github_redirect_carries_state_and_an_s256_challenge() {
        let c = client(true, "https://purestat.example");
        let url = c
            .authorize_url(Provider::GitHub, "the-state", "the-challenge")
            .unwrap();
        assert!(
            url.starts_with("https://github.com/login/oauth/authorize?"),
            "{url}"
        );
        let q = query(&url);
        assert_eq!(q["client_id"], "fake-github-client");
        assert_eq!(
            q["redirect_uri"],
            "https://purestat.example/api/oauth/github/callback"
        );
        assert_eq!(q["scope"], "user:email");
        assert_eq!(q["state"], "the-state");
        assert_eq!(q["code_challenge"], "the-challenge");
        assert_eq!(q["code_challenge_method"], "S256");
        assert!(!url.contains("fake-github-secret"));
    }

    #[test]
    fn disabled_oauth_offers_nothing_even_when_configured() {
        let c = client(false, "https://purestat.example");
        for p in Provider::ALL {
            assert!(!c.is_available(p));
            assert!(c.authorize_url(p, "s", "c").is_err());
        }
    }
}
