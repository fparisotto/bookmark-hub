use axum::routing::post;
use axum::{Extension, Json, Router};
use axum_macros::debug_handler;
use shared::{BookmarkTaskSearchRequest, BookmarkTaskSearchResponse};
use tracing::info;

use super::Claim;
use crate::db::bookmark_task::search;
use crate::error::Result;
use crate::AppContext;

pub fn routes() -> Router {
    Router::new().route("/tasks", post(search_tasks))
}

#[debug_handler]
async fn search_tasks(
    claims: Claim,
    Extension(app_context): Extension<AppContext>,
    Json(input): Json<BookmarkTaskSearchRequest>,
) -> Result<Json<BookmarkTaskSearchResponse>> {
    info!("REQUEST: {:?}", input);
    let result = search(&app_context.pool, claims.user_id, &input).await?;
    Ok(Json(result))
}
