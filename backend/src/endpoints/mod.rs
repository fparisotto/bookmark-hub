use crate::{
    db,
    error::{Error, Result},
    AppContext,
};
use axum::{routing, Extension, Json, Router};

mod auth;
mod bookmark;
mod search;
mod static_content;

async fn health_check_handler(
    Extension(app_context): Extension<AppContext>,
) -> Result<Json<String>> {
    db::run_health_check(&app_context.db).await?;
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

pub use static_content::routes as static_content;
