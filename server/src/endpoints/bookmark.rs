use axum::extract::Path;
use axum::http::StatusCode;
use axum::Json;
use axum::{routing::get, routing::post, Extension, Router};
use axum_macros::debug_handler;
use shared::{Bookmark, Bookmarks, NewBookmark, TagCount, TagOperation, Tags, TagsWithCounters};
use tracing::error;

use crate::db::bookmark;
use crate::db::task::{self, Task};
use crate::endpoints::Error;
use crate::error::Result;
use crate::AppContext;

use super::Claim;

pub fn routes() -> Router {
    Router::new()
        .route("/tags", get(get_all_tags))
        .route("/tags/:tag", get(get_bookmarks_by_tag))
        .route("/bookmarks", get(get_bookmarks).post(new_bookmark))
        .route("/bookmarks/:id", get(get_bookmark))
        .route("/bookmarks/:id/tags", post(set_tags).patch(append_tags))
}

#[debug_handler]
async fn get_bookmarks(
    claims: Claim,
    Extension(app_context): Extension<AppContext>,
) -> Result<Json<Bookmarks>> {
    let bookmarks = bookmark::get_by_user(&app_context.pool, claims.user_id).await?;
    Ok(Json(Bookmarks { bookmarks }))
}

#[debug_handler]
async fn get_all_tags(
    claims: Claim,
    Extension(app_context): Extension<AppContext>,
) -> Result<Json<TagsWithCounters>> {
    let tags = bookmark::get_tag_count_by_user(&app_context.pool, claims.user_id).await?;
    let tags = tags
        .into_iter()
        .map(|(tag, count)| TagCount { tag, count })
        .collect::<Vec<_>>();
    Ok(Json(TagsWithCounters { tags }))
}

#[debug_handler]
async fn get_bookmarks_by_tag(
    claims: Claim,
    Extension(app_context): Extension<AppContext>,
    Path(tag): Path<String>,
) -> Result<Json<Bookmarks>> {
    let bookmarks = bookmark::get_by_tag(&app_context.pool, claims.user_id, &tag).await?;
    Ok(Json(Bookmarks { bookmarks }))
}

#[debug_handler]
async fn get_bookmark(
    claims: Claim,
    Extension(app_context): Extension<AppContext>,
    Path(id): Path<String>,
) -> Result<Json<Bookmark>> {
    let maybe_bookmark =
        bookmark::get_with_user_data(&app_context.pool, claims.user_id, &id).await?;
    match maybe_bookmark {
        Some(bookmark) => Ok(Json(bookmark)),
        None => Err(Error::NotFound),
    }
}

#[debug_handler]
async fn new_bookmark(
    claims: Claim,
    Extension(app_context): Extension<AppContext>,
    Json(input): Json<NewBookmark>,
) -> Result<(StatusCode, Json<Task>)> {
    // FIXME put this validation in a better place
    let mut tags = input.tags.clone().unwrap_or_default();
    tags.retain(|t| !t.trim().is_empty());
    let response = task::create(&app_context.pool, claims.user_id, input.url, tags).await?;
    if let Err(error) = app_context.tx_new_task.send(()) {
        error!(?error, "Fail on notify new task");
    }
    Ok((StatusCode::CREATED, Json(response)))
}

#[debug_handler]
async fn set_tags(
    claims: Claim,
    Extension(app_context): Extension<AppContext>,
    Path(bookmark_id): Path<String>,
    Json(tags): Json<Tags>,
) -> Result<Json<Bookmark>> {
    let updated = bookmark::update_tags(
        &app_context.pool,
        claims.user_id,
        &bookmark_id,
        &TagOperation::Set(tags.tags),
    )
    .await?;
    Ok(Json(updated))
}

#[debug_handler]
async fn append_tags(
    claims: Claim,
    Extension(app_context): Extension<AppContext>,
    Path(bookmark_id): Path<String>,
    Json(tags): Json<Tags>,
) -> Result<Json<Bookmark>> {
    let updated = bookmark::update_tags(
        &app_context.pool,
        claims.user_id,
        &bookmark_id,
        &TagOperation::Append(tags.tags),
    )
    .await?;
    Ok(Json(updated))
}
