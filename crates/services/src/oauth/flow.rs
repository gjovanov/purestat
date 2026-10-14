//! The CSRF `state` and the PKCE verifier of one sign-in, bound to the browser
//! that started it.
//!
//! [`OAuthFlows::begin`] mints three independent 256-bit random values: the
//! flow id, which goes in the browser's cookie; the `state`, which goes to the
//! provider and comes back on the callback; and the PKCE `code_verifier`,
//! which stays on the server (the provider sees only its S256 challenge). It
//! stores `{provider, state, verifier, issued_at}` under a hash of the flow id
//! for [`FLOW_TTL_SECS`].
//!
//! [`OAuthFlows::finish`] takes that record out of the store first (GET and
//! DEL in one transaction, so a flow is used once, whatever happens next),
//! then checks the provider, the returned `state` (in constant time) and the
//! age, and hands back the verifier for the code exchange.
//!
//! Why this closes login CSRF: an attacker can start a flow and obtain a valid
//! `code` and `state` of their own, but cannot place their flow cookie in a
//! victim's browser, so a callback URL they send the victim fails here.

use std::future::Future;
use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::TryRngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use super::Provider;

/// How long a sign-in may take from the redirect to the callback, in seconds.
pub const FLOW_TTL_SECS: i64 = 600;

/// How far a record's `issued_at` may lie in the future (clocks of two API
/// replicas), in seconds.
const CLOCK_SKEW_SECS: i64 = 60;

/// The length of a flow id, state or verifier: 32 random bytes, base64url
/// without padding.
const TOKEN_LEN: usize = 43;

/// Past this, a store call is given up and the sign-in refused as unavailable.
const STORE_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Serialize, Deserialize)]
struct FlowRecord {
    provider: String,
    state: String,
    verifier: String,
    issued_at: i64,
}

/// A started sign-in: `flow_id` goes in the browser's cookie, `state` and
/// `code_challenge` in the redirect to the provider.
#[derive(Debug)]
pub struct BegunFlow {
    pub flow_id: String,
    pub state: String,
    pub code_challenge: String,
}

/// Why a callback's state was refused. Every reason means "start again"; they
/// differ only in the log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowRefusal {
    /// No flow cookie, or one this server could not have issued.
    NoCookie,
    /// The callback carries no `state`.
    NoState,
    /// No record for the cookie: used already, expired, or never issued.
    Unknown,
    /// The record is older than [`FLOW_TTL_SECS`].
    Expired,
    /// The flow was started for another provider.
    WrongProvider,
    /// The returned `state` is not the one this browser was given.
    StateMismatch,
}

#[derive(Debug, thiserror::Error)]
pub enum FlowError {
    #[error("no randomness from the operating system: {0}")]
    Random(String),
    #[error("flow store: {0}")]
    Store(String),
}

/// Why [`OAuthFlows::finish`] gave no verifier.
#[derive(Debug)]
pub enum FlowFailure {
    /// The callback does not belong to a flow this browser started.
    Refused(FlowRefusal),
    /// The store could not answer; nothing is known about the flow.
    Unavailable(FlowError),
}

/// Where flow records live. A record is put once and taken at most once.
pub trait FlowStore: Send + Sync {
    /// Store `value` under `key` for `ttl_secs` seconds. Never overwrites.
    fn put(
        &self,
        key: &str,
        value: String,
        ttl_secs: u64,
    ) -> impl Future<Output = Result<(), FlowError>> + Send;

    /// Remove the value under `key` and return it, atomically: of two calls
    /// with one key, at most one gets the value.
    fn take(&self, key: &str) -> impl Future<Output = Result<Option<String>, FlowError>> + Send;
}

/// Starts and finishes sign-ins against a [`FlowStore`].
pub struct OAuthFlows<S> {
    store: S,
}

impl<S: FlowStore> OAuthFlows<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }

    /// Start a sign-in with `provider` at `now` (Unix seconds).
    pub async fn begin(&self, provider: Provider, now: i64) -> Result<BegunFlow, FlowError> {
        let flow_id = random_token()?;
        let state = random_token()?;
        let verifier = random_token()?;
        let code_challenge = code_challenge(&verifier);
        let record = FlowRecord {
            provider: provider.slug().to_string(),
            state: state.clone(),
            verifier,
            issued_at: now,
        };
        let value = serde_json::to_string(&record).map_err(|e| FlowError::Store(e.to_string()))?;
        self.store
            .put(&store_key(&flow_id), value, FLOW_TTL_SECS as u64)
            .await?;
        Ok(BegunFlow {
            flow_id,
            state,
            code_challenge,
        })
    }

    /// Finish the sign-in the browser's flow cookie names: consume its record
    /// and check the callback against it. Returns the PKCE verifier to send
    /// in the code exchange.
    pub async fn finish(
        &self,
        flow_id: Option<&str>,
        provider: Provider,
        returned_state: Option<&str>,
        now: i64,
    ) -> Result<String, FlowFailure> {
        let Some(flow_id) = flow_id.filter(|id| is_token(id)) else {
            return Err(FlowFailure::Refused(FlowRefusal::NoCookie));
        };
        // Taken before anything is checked, so whatever the outcome, this
        // flow cannot be used again.
        let value = self
            .store
            .take(&store_key(flow_id))
            .await
            .map_err(FlowFailure::Unavailable)?
            .ok_or(FlowFailure::Refused(FlowRefusal::Unknown))?;
        let record: FlowRecord =
            serde_json::from_str(&value).map_err(|_| FlowFailure::Refused(FlowRefusal::Unknown))?;
        check(&record, provider, returned_state, now).map_err(FlowFailure::Refused)?;
        Ok(record.verifier)
    }
}

fn check(
    record: &FlowRecord,
    provider: Provider,
    returned_state: Option<&str>,
    now: i64,
) -> Result<(), FlowRefusal> {
    if record.provider != provider.slug() {
        return Err(FlowRefusal::WrongProvider);
    }
    let Some(returned) = returned_state.filter(|s| !s.is_empty()) else {
        return Err(FlowRefusal::NoState);
    };
    if !bool::from(record.state.as_bytes().ct_eq(returned.as_bytes())) {
        return Err(FlowRefusal::StateMismatch);
    }
    let age = now - record.issued_at;
    if !(-CLOCK_SKEW_SECS..=FLOW_TTL_SECS).contains(&age) {
        return Err(FlowRefusal::Expired);
    }
    Ok(())
}

/// The PKCE S256 challenge of a verifier: BASE64URL(SHA-256(verifier)),
/// RFC 7636 section 4.2.
pub fn code_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

/// 32 bytes from the operating system's generator, base64url. As a PKCE
/// verifier that is 43 characters of the unreserved set, as RFC 7636 asks.
fn random_token() -> Result<String, FlowError> {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng
        .try_fill_bytes(&mut bytes)
        .map_err(|e| FlowError::Random(e.to_string()))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

/// Whether `s` has the shape of a [`random_token`].
fn is_token(s: &str) -> bool {
    s.len() == TOKEN_LEN
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// The store key of a flow: a hash of its id, so the store never holds the
/// value a browser presents.
fn store_key(flow_id: &str) -> String {
    format!(
        "purestat:oauth:flow:{}",
        hex::encode(Sha256::digest(flow_id.as_bytes()))
    )
}

/// Flow records in Redis, which every API replica shares. A callback may
/// reach another replica than the redirect did.
#[derive(Clone)]
pub struct RedisFlowStore {
    redis: redis::aio::ConnectionManager,
}

impl RedisFlowStore {
    pub fn new(redis: redis::aio::ConnectionManager) -> Self {
        Self { redis }
    }
}

impl FlowStore for RedisFlowStore {
    async fn put(&self, key: &str, value: String, ttl_secs: u64) -> Result<(), FlowError> {
        let mut conn = self.redis.clone();
        let mut cmd = redis::cmd("SET");
        cmd.arg(key).arg(value).arg("NX").arg("EX").arg(ttl_secs);
        let set = cmd.query_async::<bool>(&mut conn);
        match tokio::time::timeout(STORE_TIMEOUT, set).await {
            Ok(Ok(true)) => Ok(()),
            Ok(Ok(false)) => Err(FlowError::Store("the flow id is taken".to_string())),
            Ok(Err(e)) => Err(FlowError::Store(e.to_string())),
            Err(_) => Err(FlowError::Store("timed out".to_string())),
        }
    }

    async fn take(&self, key: &str) -> Result<Option<String>, FlowError> {
        let mut conn = self.redis.clone();
        // MULTI/EXEC: the read and the delete happen together, so two
        // callbacks presenting one cookie cannot both read the record.
        let mut pipe = redis::pipe();
        pipe.atomic().get(key).del(key);
        let taken = pipe.query_async::<(Option<String>, i64)>(&mut conn);
        match tokio::time::timeout(STORE_TIMEOUT, taken).await {
            Ok(Ok((value, _deleted))) => Ok(value),
            Ok(Err(e)) => Err(FlowError::Store(e.to_string())),
            Err(_) => Err(FlowError::Store("timed out".to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use super::*;

    /// A store in memory with the same contract: put never overwrites, take
    /// removes. The Redis store is exercised by the integration test
    /// (purestat-tests, oauth_test).
    #[derive(Default)]
    struct MemoryStore {
        values: Mutex<HashMap<String, String>>,
    }

    impl MemoryStore {
        fn len(&self) -> usize {
            self.values.lock().unwrap().len()
        }
    }

    impl FlowStore for MemoryStore {
        async fn put(&self, key: &str, value: String, _ttl_secs: u64) -> Result<(), FlowError> {
            let mut values = self.values.lock().unwrap();
            if values.contains_key(key) {
                return Err(FlowError::Store("the flow id is taken".to_string()));
            }
            values.insert(key.to_string(), value);
            Ok(())
        }

        async fn take(&self, key: &str) -> Result<Option<String>, FlowError> {
            Ok(self.values.lock().unwrap().remove(key))
        }
    }

    /// A store that is down.
    struct DownStore;

    impl FlowStore for DownStore {
        async fn put(&self, _: &str, _: String, _: u64) -> Result<(), FlowError> {
            Err(FlowError::Store("connection refused".to_string()))
        }

        async fn take(&self, _: &str) -> Result<Option<String>, FlowError> {
            Err(FlowError::Store("connection refused".to_string()))
        }
    }

    const T0: i64 = 1_760_000_000;

    fn flows() -> OAuthFlows<MemoryStore> {
        OAuthFlows::new(MemoryStore::default())
    }

    fn refused(r: Result<String, FlowFailure>) -> FlowRefusal {
        match r {
            Err(FlowFailure::Refused(why)) => why,
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    #[test]
    fn the_s256_challenge_matches_rfc_7636_appendix_b() {
        assert_eq!(
            code_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[tokio::test]
    async fn begin_mints_unguessable_independent_values() {
        let flows = flows();
        let a = flows.begin(Provider::Google, T0).await.unwrap();
        let b = flows.begin(Provider::Google, T0).await.unwrap();
        for v in [&a.flow_id, &a.state, &b.flow_id, &b.state] {
            assert!(is_token(v), "{v}");
        }
        // The challenge is a SHA-256 in base64url: 43 characters, no padding.
        assert_eq!(a.code_challenge.len(), 43);
        assert!(!a.code_challenge.contains('='));
        assert_ne!(a.flow_id, a.state);
        assert_ne!(a.flow_id, b.flow_id);
        assert_ne!(a.state, b.state);
        assert_ne!(a.code_challenge, b.code_challenge);
        // The store holds a hash of the flow id, never the id itself.
        let values = flows.store.values.lock().unwrap();
        assert!(
            values
                .keys()
                .all(|k| !k.contains(&a.flow_id) && !k.contains(&b.flow_id))
        );
    }

    #[tokio::test]
    async fn the_right_state_yields_the_verifier_of_the_challenge_sent() {
        let flows = flows();
        let begun = flows.begin(Provider::GitHub, T0).await.unwrap();
        let verifier = flows
            .finish(
                Some(&begun.flow_id),
                Provider::GitHub,
                Some(&begun.state),
                T0 + 30,
            )
            .await
            .unwrap();
        // The verifier the code exchange sends is the one the provider was
        // shown the challenge of, and it is a valid RFC 7636 verifier.
        assert_eq!(code_challenge(&verifier), begun.code_challenge);
        assert!((43..=128).contains(&verifier.len()));
        assert_ne!(verifier, begun.state);
    }

    #[tokio::test]
    async fn a_flow_is_used_once() {
        let flows = flows();
        let begun = flows.begin(Provider::Google, T0).await.unwrap();
        let finish = || {
            flows.finish(
                Some(&begun.flow_id),
                Provider::Google,
                Some(&begun.state),
                T0 + 5,
            )
        };
        assert!(finish().await.is_ok());
        assert_eq!(refused(finish().await), FlowRefusal::Unknown);
    }

    #[tokio::test]
    async fn a_mismatched_state_is_refused_and_spends_the_flow() {
        let flows = flows();
        let begun = flows.begin(Provider::Google, T0).await.unwrap();
        let other = flows.begin(Provider::Google, T0).await.unwrap();
        // An attacker's own state (from a flow they started) does not match
        // the flow in the victim's cookie.
        assert_eq!(
            refused(
                flows
                    .finish(
                        Some(&begun.flow_id),
                        Provider::Google,
                        Some(&other.state),
                        T0 + 5
                    )
                    .await
            ),
            FlowRefusal::StateMismatch
        );
        // Neither does a near miss of the right one: its last character changed.
        let fresh = flows.begin(Provider::Google, T0).await.unwrap();
        let mut near = fresh.state.clone();
        let last = near.pop().unwrap();
        near.push(if last == 'A' { 'B' } else { 'A' });
        assert_eq!(
            refused(
                flows
                    .finish(Some(&fresh.flow_id), Provider::Google, Some(&near), T0 + 5)
                    .await
            ),
            FlowRefusal::StateMismatch
        );
        // The refused flow is gone: the right state no longer works either.
        assert_eq!(
            refused(
                flows
                    .finish(
                        Some(&begun.flow_id),
                        Provider::Google,
                        Some(&begun.state),
                        T0 + 6
                    )
                    .await
            ),
            FlowRefusal::Unknown
        );
    }

    #[tokio::test]
    async fn a_missing_state_or_cookie_is_refused() {
        let flows = flows();
        let begun = flows.begin(Provider::Google, T0).await.unwrap();
        assert_eq!(
            refused(
                flows
                    .finish(None, Provider::Google, Some(&begun.state), T0)
                    .await
            ),
            FlowRefusal::NoCookie
        );
        // A cookie value no flow could have: not even looked up.
        assert_eq!(
            refused(
                flows
                    .finish(Some("not-a-flow"), Provider::Google, Some(&begun.state), T0)
                    .await
            ),
            FlowRefusal::NoCookie
        );
        assert_eq!(flows.store.len(), 1, "the flow is still there");
        assert_eq!(
            refused(
                flows
                    .finish(Some(&begun.flow_id), Provider::Google, None, T0)
                    .await
            ),
            FlowRefusal::NoState
        );
        let again = flows.begin(Provider::Google, T0).await.unwrap();
        assert_eq!(
            refused(
                flows
                    .finish(Some(&again.flow_id), Provider::Google, Some(""), T0)
                    .await
            ),
            FlowRefusal::NoState
        );
    }

    #[tokio::test]
    async fn an_unknown_flow_id_is_refused() {
        let flows = flows();
        let begun = flows.begin(Provider::Google, T0).await.unwrap();
        let unknown = random_token().unwrap();
        assert_eq!(
            refused(
                flows
                    .finish(Some(&unknown), Provider::Google, Some(&begun.state), T0)
                    .await
            ),
            FlowRefusal::Unknown
        );
    }

    #[tokio::test]
    async fn a_flow_expires_after_ten_minutes() {
        let flows = flows();
        let begun = flows.begin(Provider::Google, T0).await.unwrap();
        assert_eq!(
            refused(
                flows
                    .finish(
                        Some(&begun.flow_id),
                        Provider::Google,
                        Some(&begun.state),
                        T0 + FLOW_TTL_SECS + 1
                    )
                    .await
            ),
            FlowRefusal::Expired
        );
        // The last second still counts.
        let edge = flows.begin(Provider::Google, T0).await.unwrap();
        assert!(
            flows
                .finish(
                    Some(&edge.flow_id),
                    Provider::Google,
                    Some(&edge.state),
                    T0 + FLOW_TTL_SECS
                )
                .await
                .is_ok()
        );
        // A record from the far future is no better than an old one.
        let future = flows.begin(Provider::Google, T0 + 3600).await.unwrap();
        assert_eq!(
            refused(
                flows
                    .finish(
                        Some(&future.flow_id),
                        Provider::Google,
                        Some(&future.state),
                        T0
                    )
                    .await
            ),
            FlowRefusal::Expired
        );
    }

    #[tokio::test]
    async fn a_flow_finishes_only_with_the_provider_it_started_with() {
        let flows = flows();
        let begun = flows.begin(Provider::GitHub, T0).await.unwrap();
        assert_eq!(
            refused(
                flows
                    .finish(
                        Some(&begun.flow_id),
                        Provider::Google,
                        Some(&begun.state),
                        T0
                    )
                    .await
            ),
            FlowRefusal::WrongProvider
        );
    }

    #[tokio::test]
    async fn a_store_outage_is_unavailable_not_a_refusal() {
        let flows = OAuthFlows::new(DownStore);
        assert!(matches!(
            flows.begin(Provider::Google, T0).await,
            Err(FlowError::Store(_))
        ));
        let id = random_token().unwrap();
        assert!(matches!(
            flows
                .finish(Some(&id), Provider::Google, Some("x"), T0)
                .await,
            Err(FlowFailure::Unavailable(_))
        ));
    }
}
