use axum::http::header::WWW_AUTHENTICATE;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use std::borrow::Cow;
use std::collections::HashMap;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(serde::Serialize)]
struct ErrorsPayload {
    errors: HashMap<Cow<'static, str>, Vec<Cow<'static, str>>>,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("authentication required")]
    Unauthorized,
    #[error("action_not_allowed")]
    Forbidden,
    #[error("request path not found")]
    NotFound,
    #[error("invalid_payload")]
    UnprocessableEntity {
        errors: HashMap<Cow<'static, str>, Vec<Cow<'static, str>>>,
    },
    #[error("invalid_payload")]
    BadRequest {
        errors: HashMap<Cow<'static, str>, Vec<Cow<'static, str>>>,
    },
    #[error("database_error")]
    Database(#[from] sqlx::Error),
    #[error("Migration error")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("constraint_violation")]
    ConstraintViolation { constraint: String, message: String },
    #[error("internal_server_error")]
    Anyhow(#[from] anyhow::Error),
    #[error("wrong_credentials")]
    WrongCredentials,
    #[error("missing_credentials")]
    MissingCredentials,
    #[error("invalid_token")]
    InvalidToken,
    #[error("internal_error")]
    Jwt(#[from] jsonwebtoken::errors::Error),
    #[error("internal_error")]
    Argon2 { details: String },
}

impl Error {
    pub fn unprocessable_entity<K, V>(errors: impl IntoIterator<Item = (K, V)>) -> Self
    where
        K: Into<Cow<'static, str>>,
        V: Into<Cow<'static, str>>,
    {
        let mut error_map = HashMap::new();

        for (key, val) in errors {
            error_map
                .entry(key.into())
                .or_insert_with(Vec::new)
                .push(val.into());
        }

        Self::UnprocessableEntity { errors: error_map }
    }

    pub fn bad_request<K, V>(errors: impl IntoIterator<Item = (K, V)>) -> Self
    where
        K: Into<Cow<'static, str>>,
        V: Into<Cow<'static, str>>,
    {
        let mut error_map = HashMap::new();

        for (key, val) in errors {
            error_map
                .entry(key.into())
                .or_insert_with(Vec::new)
                .push(val.into());
        }

        Self::BadRequest { errors: error_map }
    }

    pub fn constraint_violation(constraint: &str, message: &str) -> Self {
        Self::ConstraintViolation {
            constraint: constraint.to_string(),
            message: message.to_string(),
        }
    }

    pub fn argon2(details: String) -> Self {
        Self::Argon2 { details }
    }

    fn status_code(&self) -> StatusCode {
        match self {
            Error::Unauthorized => StatusCode::UNAUTHORIZED,
            Error::WrongCredentials => StatusCode::UNAUTHORIZED,
            Error::Forbidden => StatusCode::FORBIDDEN,
            Error::NotFound => StatusCode::NOT_FOUND,
            Error::UnprocessableEntity { .. } => StatusCode::UNPROCESSABLE_ENTITY,
            Error::ConstraintViolation { .. } => StatusCode::BAD_REQUEST,
            Error::BadRequest { .. } => StatusCode::BAD_REQUEST,
            Error::MissingCredentials => StatusCode::BAD_REQUEST,
            Error::InvalidToken => StatusCode::BAD_REQUEST,
            Error::Argon2 { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            Error::Jwt(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::Anyhow(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::Migration(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        match self {
            Self::BadRequest { errors } => {
                let t = (StatusCode::BAD_REQUEST, Json(ErrorsPayload { errors }));
                return t.into_response();
            }
            Self::UnprocessableEntity { errors } => {
                let t = (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(ErrorsPayload { errors }),
                );
                return t.into_response();
            }
            Self::Unauthorized => {
                let t = (
                    self.status_code(),
                    [(WWW_AUTHENTICATE, HeaderValue::from_static("Token"))]
                        .into_iter()
                        .collect::<HeaderMap>(),
                    self.to_string(),
                );
                return t.into_response();
            }
            Self::Database(ref e) => {
                tracing::error!("Database error: {:?}", e);
            }
            Self::Anyhow(ref e) => {
                tracing::error!("Generic error: {:?}", e);
            }

            _ => (),
        }

        (self.status_code(), self.to_string()).into_response()
    }
}
