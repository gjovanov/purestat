use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use purestat_services::auth::AuthError;
use purestat_services::DaoError;
use serde::Serialize;

#[derive(Debug)]
pub enum ApiError {
    NotFound(String),
    BadRequest(String),
    Unauthorized(String),
    Forbidden(String),
    Conflict(String),
    Internal(String),
    Validation(String),
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
    message: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error_type, message) = match self {
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, "not_found", msg),
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, "bad_request", msg),
            ApiError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, "unauthorized", msg),
            ApiError::Forbidden(msg) => (StatusCode::FORBIDDEN, "forbidden", msg),
            ApiError::Conflict(msg) => (StatusCode::CONFLICT, "conflict", msg),
            ApiError::Internal(msg) => {
                tracing::error!(error = %msg, "Internal server error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal",
                    "Internal server error".to_string(),
                )
            }
            ApiError::Validation(msg) => {
                (StatusCode::UNPROCESSABLE_ENTITY, "validation", msg)
            }
        };

        let body = ErrorResponse {
            error: error_type.to_string(),
            message,
        };

        (status, Json(body)).into_response()
    }
}

impl From<DaoError> for ApiError {
    fn from(err: DaoError) -> Self {
        match err {
            DaoError::NotFound => ApiError::NotFound("Entity not found".to_string()),
            DaoError::DuplicateKey(msg) => ApiError::Conflict(msg),
            DaoError::Forbidden(msg) => ApiError::Forbidden(msg),
            DaoError::Validation(msg) => ApiError::Validation(msg),
            DaoError::Mongo(e) => ApiError::Internal(e.to_string()),
            DaoError::BsonSer(e) => ApiError::Internal(e.to_string()),
            DaoError::BsonDe(e) => ApiError::Internal(e.to_string()),
        }
    }
}

impl From<AuthError> for ApiError {
    fn from(err: AuthError) -> Self {
        match err {
            AuthError::InvalidCredentials => {
                ApiError::Unauthorized("Invalid credentials".to_string())
            }
            AuthError::TokenExpired => {
                ApiError::Unauthorized("Token expired".to_string())
            }
            AuthError::InvalidToken(msg) => ApiError::Unauthorized(msg),
            AuthError::HashError(msg) => ApiError::Internal(msg),
        }
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(err: anyhow::Error) -> Self {
        ApiError::Internal(err.to_string())
    }
}
