use std::net::SocketAddr;

use axum::extract::ConnectInfo;
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use axum_macros::debug_handler;
use chrono::{Duration, Utc};
use secrecy::ExposeSecret;
use shared::{SignInRequest, SignInResponse, SignUpRequest, SignUpResponse, UserProfile};
use tracing::{debug, error, info, warn};

use super::Claim;
use crate::auth_rate_limit::AuthRateLimitKey;
use crate::db::user;
use crate::error::{Error, Result};
use crate::AppContext;

const MIN_PASSWORD_LENGTH: usize = 8;

fn validate_signup(payload: &SignUpRequest) -> Result<()> {
    let mut errors: Vec<(&'static str, &'static str)> = Vec::new();
    if payload.username.trim().is_empty() {
        errors.push(("username", "username must not be empty"));
    }
    if payload.password.expose_secret().trim().is_empty() {
        errors.push(("password", "password must not be empty"));
    } else if payload.password.expose_secret().chars().count() < MIN_PASSWORD_LENGTH {
        errors.push(("password", "password must be at least 8 characters"));
    }
    if payload
        .password
        .expose_secret()
        .ne(payload.password_confirmation.expose_secret())
    {
        errors.push(("password", "password confirmation should match"));
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(Error::unprocessable_entity(errors))
    }
}

fn validate_signin(payload: &SignInRequest) -> Result<()> {
    let mut errors: Vec<(&'static str, &'static str)> = Vec::new();
    if payload.username.trim().is_empty() {
        errors.push(("username", "username must not be empty"));
    }
    if payload.password.expose_secret().trim().is_empty() {
        errors.push(("password", "password must not be empty"));
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(Error::unprocessable_entity(errors))
    }
}

fn auth_rate_limit_key(
    action: &'static str,
    username: &str,
    client_addr: SocketAddr,
) -> AuthRateLimitKey {
    AuthRateLimitKey::new(
        action,
        client_addr.ip().to_string(),
        username.trim().to_lowercase(),
    )
}

pub fn router() -> Router {
    Router::new()
        .route("/auth/sign-up", post(sign_up))
        .route("/auth/sign-in", post(sign_in))
        .route("/auth/user-profile", get(get_user_profile))
}

#[debug_handler]
async fn get_user_profile(
    claims: Claim,
    Extension(app_context): Extension<AppContext>,
) -> Result<Json<UserProfile>> {
    debug!(
        user_id = %claims.user_id,
        username = %claims.sub,
        "Getting user profile"
    );
    match user::get_by_id(&app_context.pool, &claims.user_id).await {
        Ok(Some(user)) => {
            info!(
                user_id = %claims.user_id,
                "User profile retrieved successfully"
            );
            Ok(Json(UserProfile {
                user_id: user.user_id,
                username: user.username,
                created_at: user.created_at,
            }))
        }
        Ok(None) => {
            error!(
                user_id = %claims.user_id,
                username = %claims.sub,
                "User profile not found for valid JWT claim"
            );
            Err(Error::bad_request([("user", "user not found")]))
        }
        Err(error) => {
            error!(
                user_id = %claims.user_id,
                error = %error,
                "Database error fetching user profile"
            );
            Err(Error::bad_request([("user", "user not found")]))
        }
    }
}

#[debug_handler]
async fn sign_up(
    ConnectInfo(client_addr): ConnectInfo<SocketAddr>,
    Extension(app_context): Extension<AppContext>,
    Json(payload): Json<SignUpRequest>,
) -> Result<Json<SignUpResponse>> {
    info!(username = %payload.username, "User signup attempt");
    let rate_limit_key = auth_rate_limit_key("sign-up", &payload.username, client_addr);
    let decision = app_context.auth_rate_limiter.check(rate_limit_key.clone());
    if !decision.allowed {
        return Err(Error::too_many_requests(decision.retry_after_secs));
    }

    validate_signup(&payload)?;
    debug!(username = %payload.username, "Signup validation passed");

    let hashed_password = super::hash_password(payload.password).await?;
    let try_user = user::create(&app_context.pool, payload.username.clone(), hashed_password).await;
    match try_user {
        Ok(user) => {
            app_context.auth_rate_limiter.reset(&rate_limit_key);
            info!(
                user_id = %user.user_id,
                username = %user.username,
                "User successfully created"
            );
            Ok(Json(SignUpResponse {
                id: user.user_id,
                username: user.username,
            }))
        }
        Err(Error::ConstraintViolation {
            constraint,
            message: _,
        }) if constraint.eq("unique_username") => {
            warn!(
                username = %payload.username,
                "Signup failed - username already exists"
            );
            Err(Error::bad_request([(
                "username",
                "username already created",
            )]))
        }
        Err(error) => {
            error!(username = %payload.username, error = %error, "Signup failed");
            Err(error)
        }
    }
}

#[debug_handler()]
async fn sign_in(
    ConnectInfo(client_addr): ConnectInfo<SocketAddr>,
    Extension(app_context): Extension<AppContext>,
    Json(payload): Json<SignInRequest>,
) -> Result<Json<SignInResponse>> {
    info!(username = %payload.username, "User signin attempt");
    let rate_limit_key = auth_rate_limit_key("sign-in", &payload.username, client_addr);
    let decision = app_context.auth_rate_limiter.check(rate_limit_key.clone());
    if !decision.allowed {
        return Err(Error::too_many_requests(decision.retry_after_secs));
    }

    validate_signin(&payload)?;
    debug!(username = %payload.username, "Signin validation passed");

    let maybe_user = user::get_by_username(&app_context.pool, payload.username.clone()).await?;
    if let Some(user) = maybe_user {
        debug!(username = %user.username, "User found for signin");
        super::verify_password(payload.password, user.password_hash.clone()).await?;
        debug!(
            username = %user.username,
            "Password verification successful"
        );

        let expiration = Utc::now()
            .checked_add_signed(Duration::weeks(2))
            .expect("Not overflow")
            .timestamp();
        let claims = Claim {
            user_id: user.user_id,
            sub: user.username.clone(),
            exp: expiration,
        };
        let token = super::encode_token(&app_context.config, &claims)?;
        info!(
            user_id = %user.user_id,
            username = %claims.sub,
            "User successfully authenticated"
        );

        let login_response = SignInResponse {
            user_id: user.user_id,
            username: user.username,
            access_token: token,
            token_type: "Bearer".to_owned(),
        };
        app_context.auth_rate_limiter.reset(&rate_limit_key);
        return Ok(Json(login_response));
    }

    warn!(
        username = %payload.username,
        "Signin failed - user not found or wrong credentials"
    );
    Err(Error::WrongCredentials)
}

#[cfg(test)]
mod tests {
    use secrecy::SecretString;
    use shared::SignUpRequest;

    use super::validate_signup;

    #[test]
    fn signup_requires_minimum_password_length() {
        let result = validate_signup(&SignUpRequest {
            username: "alice".into(),
            password: SecretString::from("short"),
            password_confirmation: SecretString::from("short"),
        });

        assert!(result.is_err());
    }
}
