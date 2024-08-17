use axum::{routing::get, routing::post, Extension, Json, Router};
use axum_macros::debug_handler;
use chrono::{DateTime, Duration, Utc};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::{self, Claim};
use crate::database::user;
use crate::error::{Error, Result};
use crate::AppContext;

#[derive(Debug, Deserialize)]
struct SignUpPayload {
    email: String,
    password: SecretString,
    password_confirmation: SecretString,
}

impl SignUpPayload {
    fn validate(&self) -> Result<()> {
        let mut errors: Vec<(&'static str, &'static str)> = Vec::new();
        if self.email.trim().is_empty() {
            errors.push(("email", "email must not be empty"));
        }
        if self.password.expose_secret().trim().is_empty() {
            errors.push(("password", "password must not be empty"));
        }
        if self
            .password
            .expose_secret()
            .ne(self.password_confirmation.expose_secret())
        {
            errors.push(("password", "password confirmation should match"));
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(Error::unprocessable_entity(errors))
        }
    }
}

#[derive(Debug, Serialize)]
struct SignUpResponse {
    id: Uuid,
    email: String,
}

#[derive(Debug, Deserialize)]
struct SignInPayload {
    email: String,
    password: SecretString,
}

impl SignInPayload {
    fn validate(&self) -> Result<()> {
        let mut errors: Vec<(&'static str, &'static str)> = Vec::new();
        if self.email.trim().is_empty() {
            errors.push(("email", "email must not be empty"));
        }
        if self.password.expose_secret().trim().is_empty() {
            errors.push(("password", "password must not be empty"));
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(Error::unprocessable_entity(errors))
        }
    }
}

#[derive(Debug, Serialize)]
struct AuthBody {
    user_id: Uuid,
    email: String,
    access_token: String,
    token_type: String,
}

impl AuthBody {
    fn new(user: &user::User, access_token: String) -> Self {
        Self {
            user_id: user.user_id,
            email: user.email.clone(),
            access_token,
            token_type: "Bearer".to_owned(),
        }
    }
}

#[derive(Debug, Serialize)]
struct UserProfile {
    user_id: Uuid,
    email: String,
    created_at: DateTime<Utc>,
}

pub fn router() -> Router {
    Router::new()
        .route("/auth/sign-up", post(sign_up))
        .route("/auth/sign-in", post(sign_in))
        .route("/auth/user-profile", get(get_user_profile))
}

#[debug_handler()]
async fn get_user_profile(
    claims: Claim,
    Extension(app_context): Extension<AppContext>,
) -> Result<Json<UserProfile>> {
    match user::get_by_id(&app_context.db, &claims.user_id).await {
        Ok(Some(user)) => Ok(Json(UserProfile {
            user_id: user.user_id,
            email: user.email,
            created_at: user.created_at,
        })),
        Ok(None) => {
            tracing::error!(
                "Fail to fetch user, not found for given claim, user_id={}",
                &claims.user_id
            );
            Err(Error::bad_request([("user", "user not found")]))
        }
        Err(error) => {
            tracing::error!("Fail to fetch user, error={}", error);
            Err(Error::bad_request([("user", "user not found")]))
        }
    }
}

#[debug_handler()]
async fn sign_up(
    Extension(app_context): Extension<AppContext>,
    Json(payload): Json<SignUpPayload>,
) -> Result<Json<SignUpResponse>> {
    payload.validate()?;
    let hashed_password = auth::hash_password(payload.password).await?;
    let try_user = user::create(&app_context.db, payload.email, hashed_password).await;
    match try_user {
        Ok(user) => Ok(Json(SignUpResponse {
            id: user.user_id,
            email: user.email,
        })),
        Err(Error::ConstraintViolation {
            constraint,
            message: _,
        }) if constraint.eq("unique_email") => {
            Err(Error::bad_request([("email", "email already created")]))
        }
        Err(error) => Err(error),
    }
}

#[debug_handler()]
async fn sign_in(
    Extension(app_context): Extension<AppContext>,
    Json(payload): Json<SignInPayload>,
) -> Result<Json<AuthBody>> {
    payload.validate()?;
    let maybe_user = user::get_by_email(&app_context.db, &payload.email).await?;
    if let Some(user) = maybe_user {
        auth::verify_password(payload.password, user.password_hash.clone()).await?;
        let expiration = Utc::now()
            .checked_add_signed(Duration::weeks(2))
            .expect("Not overflow")
            .timestamp();
        let claims = Claim {
            user_id: user.user_id,
            sub: user.email.clone(),
            exp: expiration,
        };
        let token = auth::encode_token(&app_context.config, &claims)?;
        tracing::info!("User authenticated, email={}", &claims.sub);
        return Ok(Json(AuthBody::new(&user, token)));
    }
    Err(Error::WrongCredentials)
}
