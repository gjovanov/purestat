use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use purestat_config::JwtSettings;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Invalid credentials")]
    InvalidCredentials,
    #[error("Token expired")]
    TokenExpired,
    #[error("Invalid token: {0}")]
    InvalidToken(String),
    #[error("Password hashing error: {0}")]
    HashError(String),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub email: String,
    pub username: String,
    pub exp: i64,
    pub iat: i64,
    pub iss: String,
    pub token_type: String,
}

pub struct AuthService {
    settings: JwtSettings,
}

impl AuthService {
    pub fn new(settings: JwtSettings) -> Self {
        Self { settings }
    }

    pub fn hash_password(&self, password: &str) -> Result<String, AuthError> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| AuthError::HashError(e.to_string()))?;
        Ok(hash.to_string())
    }

    pub fn verify_password(&self, password: &str, hash: &str) -> Result<bool, AuthError> {
        let parsed = PasswordHash::new(hash)
            .map_err(|e| AuthError::HashError(e.to_string()))?;
        Ok(Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok())
    }

    pub fn generate_access_token(
        &self,
        user_id: &str,
        email: &str,
        username: &str,
    ) -> Result<String, AuthError> {
        let now = Utc::now().timestamp();
        let claims = Claims {
            sub: user_id.to_string(),
            email: email.to_string(),
            username: username.to_string(),
            exp: now + self.settings.access_token_ttl_secs as i64,
            iat: now,
            iss: self.settings.issuer.clone(),
            token_type: "access".to_string(),
        };
        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.settings.secret.as_bytes()),
        )
        .map_err(|e| AuthError::InvalidToken(e.to_string()))
    }

    pub fn generate_refresh_token(
        &self,
        user_id: &str,
        email: &str,
        username: &str,
    ) -> Result<String, AuthError> {
        let now = Utc::now().timestamp();
        let claims = Claims {
            sub: user_id.to_string(),
            email: email.to_string(),
            username: username.to_string(),
            exp: now + self.settings.refresh_token_ttl_secs as i64,
            iat: now,
            iss: self.settings.issuer.clone(),
            token_type: "refresh".to_string(),
        };
        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.settings.secret.as_bytes()),
        )
        .map_err(|e| AuthError::InvalidToken(e.to_string()))
    }

    pub fn verify_access_token(&self, token: &str) -> Result<Claims, AuthError> {
        let mut validation = Validation::default();
        validation.set_issuer(&[&self.settings.issuer]);

        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.settings.secret.as_bytes()),
            &validation,
        )
        .map_err(|e| match e.kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => AuthError::TokenExpired,
            _ => AuthError::InvalidToken(e.to_string()),
        })?;

        if token_data.claims.token_type != "access" {
            return Err(AuthError::InvalidToken(
                "Not an access token".to_string(),
            ));
        }

        Ok(token_data.claims)
    }

    pub fn verify_refresh_token(&self, token: &str) -> Result<Claims, AuthError> {
        let mut validation = Validation::default();
        validation.set_issuer(&[&self.settings.issuer]);

        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.settings.secret.as_bytes()),
            &validation,
        )
        .map_err(|e| match e.kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => AuthError::TokenExpired,
            _ => AuthError::InvalidToken(e.to_string()),
        })?;

        if token_data.claims.token_type != "refresh" {
            return Err(AuthError::InvalidToken(
                "Not a refresh token".to_string(),
            ));
        }

        Ok(token_data.claims)
    }
}
