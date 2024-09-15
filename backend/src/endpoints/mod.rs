use crate::{
    db,
    error::{Error, Result},
    AppContext, Config,
};
use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHash};
use axum::{async_trait, extract::FromRequestParts, http::request::Parts, RequestPartsExt};
use axum::{routing, Extension, Json, Router};
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};
use jsonwebtoken::{decode, encode, Header, Validation};
use jsonwebtoken::{DecodingKey, EncodingKey};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

mod auth;
mod bookmark;
mod search;
mod static_content;

pub use static_content::routes as static_content;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claim {
    pub sub: String,
    pub exp: i64,
    pub user_id: Uuid,
}

async fn health_check_handler(
    Extension(app_context): Extension<AppContext>,
) -> Result<Json<String>> {
    db::run_health_check(&app_context.pool).await?;
    Ok(Json("OK".to_string()))
}

pub fn health_check() -> Router {
    Router::new().route("/health", routing::get(health_check_handler))
}

pub fn routers_v1() -> Router {
    auth::router()
        .merge(bookmark::routes())
        .merge(search::routes())
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

fn encode_token(config: &Config, claims: &Claim) -> Result<String> {
    let hmac_key = config.hmac_key.expose_secret();
    let encoder = EncodingKey::from_secret(hmac_key.as_bytes());
    let header = Header::default();
    let token = encode(&header, claims, &encoder)?;
    Ok(token)
}

async fn hash_password(password: SecretString) -> Result<String> {
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

async fn verify_password(password: SecretString, password_hash: String) -> Result<()> {
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
