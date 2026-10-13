use axum::extract::FromRef;
use axum::http::request::Parts;
use bson::oid::ObjectId;
use purestat_db::models::Site;
use sha2::{Digest, Sha256};

use crate::routes::v1::V1Error;
use crate::state::AppState;

/// The scope a key needs to read a site's stats.
pub const STATS_READ: &str = "stats:read";

/// A request authenticated by an `X-API-Key` header: the key is unrevoked,
/// holds `stats:read`, and its site still exists. The site is the only one
/// the request can read. Anything else is 401 `{"error":"unauthorized"}`.
#[derive(Debug, Clone)]
pub struct ApiKeyAuth {
    pub key_id: ObjectId,
    pub site: Site,
}

impl<S> axum::extract::FromRequestParts<S> for ApiKeyAuth
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = V1Error;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let app_state = AppState::from_ref(state);

        let raw = parts
            .headers
            .get("x-api-key")
            .and_then(|v| v.to_str().ok())
            .map(str::trim)
            .filter(|k| !k.is_empty())
            .ok_or(V1Error::Unauthorized)?;
        // Keys are stored as the SHA-256 of the key (routes::api_key::create).
        let key_hash = hex::encode(Sha256::digest(raw.as_bytes()));

        // A failed lookup is a 500, not a 401: a database error must not tell
        // a caller with a good key that the key is bad.
        let key = app_state
            .api_keys
            .find_by_hash(&key_hash)
            .await
            .map_err(|e| V1Error::Internal(e.to_string()))?
            .ok_or(V1Error::Unauthorized)?;
        let key_id = key.id.ok_or(V1Error::Unauthorized)?;
        if !key.scopes.iter().any(|s| s == STATS_READ) {
            return Err(V1Error::Unauthorized);
        }
        let site = app_state
            .sites
            .base
            .find_one(bson::doc! { "_id": key.site_id, "org_id": key.org_id })
            .await
            .map_err(|e| V1Error::Internal(e.to_string()))?
            .ok_or(V1Error::Unauthorized)?;

        if let Err(e) = app_state.api_keys.touch_last_used(key_id).await {
            tracing::warn!(error = %e, key_id = %key_id, "Could not record an API key's last use");
        }

        Ok(Self { key_id, site })
    }
}
