use axum::{routing, Router};

use crate::error::{Error, Result};

mod auth;
mod bookmark;
mod search;
mod static_content;

async fn test_db() -> Result<String> {
    // TODO test db connection
    Ok("OK".to_string())
}

pub fn health_check() -> Router {
    Router::new().route("/health", routing::get(test_db))
}

pub fn routers_v1() -> Router {
    auth::router()
        .merge(bookmark::routes())
        .merge(search::routes())
}

pub use static_content::routes as static_content;
