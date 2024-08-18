use axum::{routing, Router};

use crate::error::{Error, Result};

mod auth;
mod bookmark;
mod search;

async fn health_check() -> Result<String> {
    Ok("OK".to_string())
}

fn routes() -> Router {
    Router::new().route("/health-check", routing::get(health_check))
}

pub fn routers_v1() -> Router {
    auth::router()
        .merge(bookmark::routes())
        .merge(routes())
        .merge(search::routes())
}
