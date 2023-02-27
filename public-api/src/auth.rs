use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHash};
use async_trait::async_trait;
use axum::Extension;
use axum::{
    extract::{FromRequest, RequestParts, TypedHeader},
    headers::{authorization::Bearer, Authorization},
};
use jsonwebtoken::{decode, encode, Header, Validation};
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::{AppContext, Config};

use jsonwebtoken::{DecodingKey, EncodingKey};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: i64,
    pub user_id: Uuid,
}

impl Display for Claims {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Email: {}", self.sub)
    }
}

pub struct Keys {
    pub encoding: EncodingKey,
    pub decoding: DecodingKey,
}

impl Keys {
    pub fn new(secret: &[u8]) -> Self {
        Self {
            encoding: EncodingKey::from_secret(secret),
            decoding: DecodingKey::from_secret(secret),
        }
    }
}

#[async_trait]
impl<B> FromRequest<B> for Claims
where
    B: Send,
{
    type Rejection = Error;

    async fn from_request(req: &mut RequestParts<B>) -> std::result::Result<Self, Self::Rejection> {
        let app_context: Extension<AppContext> = Extension::from_request(req)
            .await
            .expect("Bug: AppState should be added as an Extension");

        let TypedHeader(Authorization(bearer)) =
            TypedHeader::<Authorization<Bearer>>::from_request(req)
                .await
                .map_err(|_| Error::InvalidToken)?;

        let token_data = decode::<Claims>(
            bearer.token(),
            &app_context.config.auth_keys.decoding,
            &Validation::default(),
        )
        .map_err(|_| Error::InvalidToken)?;

        Ok(token_data.claims)
    }
}

pub fn encode_token(config: &Config, claims: &Claims) -> Result<String> {
    let token = encode(&Header::default(), claims, &config.auth_keys.encoding)?;
    Ok(token)
}

pub async fn hash_password(password: String) -> Result<String> {
    tokio::task::spawn_blocking(move || {
        let salt = SaltString::generate(rand::thread_rng());
        match PasswordHash::generate(Argon2::default(), password, salt.as_str()) {
            Ok(hash) => Ok(hash.to_string()),
            Err(error) => Err(Error::argon2(error.to_string())),
        }
    })
    .await
    .map_err(|error| Error::argon2(error.to_string()))?
}

pub async fn verify_password(password: String, password_hash: String) -> Result<()> {
    tokio::task::spawn_blocking(move || -> Result<()> {
        let hash: PasswordHash = PasswordHash::new(&password_hash).map_err(|e| Error::Argon2 {
            details: format!("invalid password hash: {e}"),
        })?;

        hash.verify_password(&[&Argon2::default()], password)
            .map_err(|e| match e {
                argon2::password_hash::Error::Password => Error::WrongCredentials,
                _ => Error::argon2(format!("failed to verify password hash: {e}")),
            })
    })
    .await
    .map_err(|error| Error::argon2(error.to_string()))?
}
