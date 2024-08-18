use axum::Json;
use axum::{routing::post, Extension, Router};
use axum_macros::debug_handler;

use crate::auth::Claim;
use crate::db::search::{search, SearchRequest, SearchResponse};
use crate::error::Result;
use crate::AppContext;

pub fn routes() -> Router {
    Router::new().route("/search", post(search_bookmark))
}

#[debug_handler()]
async fn search_bookmark(
    claims: Claim,
    Extension(app_context): Extension<AppContext>,
    Json(input): Json<SearchRequest>,
) -> Result<Json<SearchResponse>> {
    let result = search(&app_context.db, &claims.user_id, input).await?;
    Ok(Json(result))
}
