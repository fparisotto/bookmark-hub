use axum::routing::post;
use axum::{Extension, Json, Router};
use axum_macros::debug_handler;
use shared::{SearchRequest, SearchResponse, TagFilter};

use super::Claim;
use crate::db::search::search;
use crate::error::Result;
use crate::AppContext;

pub fn routes() -> Router {
    Router::new().route("/search", post(search_bookmark))
}

#[debug_handler]
async fn search_bookmark(
    claims: Claim,
    Extension(app_context): Extension<AppContext>,
    Json(mut input): Json<SearchRequest>,
) -> Result<Json<SearchResponse>> {
    if let Some(TagFilter::And(tags) | TagFilter::Or(tags)) = &mut input.tags_filter {
        for tag in tags.iter_mut() {
            *tag = tag.to_lowercase();
        }
    }
    let result = search(&app_context.pool, claims.user_id, &input).await?;
    Ok(Json(result))
}
