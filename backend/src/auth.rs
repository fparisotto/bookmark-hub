use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHash};
use axum::Extension;
use axum::{async_trait, extract::FromRequestParts, http::request::Parts, RequestPartsExt};
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};
use jsonwebtoken::{decode, encode, Header, Validation};
use jsonwebtoken::{DecodingKey, EncodingKey};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::{AppContext, Config};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claim {
    pub sub: String,
    pub exp: i64,
    pub user_id: Uuid,
}

impl Display for Claim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Claims[email: {}]", self.sub)
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for Claim
where
    S: Send + Sync,
{
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Extension(app_context): Extension<AppContext> =
            Extension::from_request_parts(parts, state)
                .await
                .expect("Bug: AppContext should be added as an Extension");

        let TypedHeader(Authorization(bearer)) = parts
            .extract::<TypedHeader<Authorization<Bearer>>>()
            .await
            .map_err(|_| Error::InvalidToken)?;

        let hmac_key = app_context.config.hmac_key.expose_secret();
        let decoder = DecodingKey::from_secret(hmac_key.as_bytes());

        let token_data = decode::<Claim>(bearer.token(), &decoder, &Validation::default())
            .map_err(|_| Error::InvalidToken)?;

        Ok(token_data.claims)
    }
}

pub fn encode_token(config: &Config, claims: &Claim) -> Result<String> {
    let hmac_key = config.hmac_key.expose_secret();
    let encoder = EncodingKey::from_secret(hmac_key.as_bytes());
    let header = Header::default();
    let token = encode(&header, claims, &encoder)?;
    Ok(token)
}

pub async fn hash_password(password: SecretString) -> Result<String> {
    tokio::task::spawn_blocking(move || {
        let salt = SaltString::generate(rand::thread_rng());
        match PasswordHash::generate(Argon2::default(), password.expose_secret(), salt.as_salt()) {
            Ok(hash) => Ok(hash.to_string()),
            Err(error) => Err(Error::argon2(error.to_string())),
        }
    })
    .await
    .map_err(|error| Error::argon2(error.to_string()))?
}

pub async fn verify_password(password: SecretString, password_hash: String) -> Result<()> {
    tokio::task::spawn_blocking(move || -> Result<()> {
        let hash: PasswordHash = PasswordHash::new(&password_hash).map_err(|e| Error::Argon2 {
            details: format!("invalid password hash: {e}"),
        })?;

        hash.verify_password(&[&Argon2::default()], password.expose_secret())
            .map_err(|e| match e {
                argon2::password_hash::Error::Password => Error::WrongCredentials,
                _ => Error::argon2(format!("failed to verify password hash: {e}")),
            })
    })
    .await
    .map_err(|error| Error::argon2(error.to_string()))?
}
