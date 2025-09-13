use axum::extract::Path;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use axum_macros::debug_handler;
use shared::{
    Bookmark, BookmarkTask, Bookmarks, NewBookmark, TagCount, TagOperation, Tags, TagsWithCounters,
};
use tracing::{debug, error, info};

use super::Claim;
use crate::db::{bookmark, bookmark_task};
use crate::endpoints::Error;
use crate::error::Result;
use crate::AppContext;

pub fn routes() -> Router {
    Router::new()
        .route("/tags", get(get_all_tags))
        .route("/tags/{tag}", get(get_bookmarks_by_tag))
        .route("/bookmarks", get(get_bookmarks).post(new_bookmark))
        .route("/bookmarks/{id}", get(get_bookmark))
        .route("/bookmarks/{id}/tags", post(set_tags).patch(append_tags))
}

#[debug_handler]
async fn get_bookmarks(
    claims: Claim,
    Extension(app_context): Extension<AppContext>,
) -> Result<Json<Bookmarks>> {
    debug!(user_id = %claims.user_id, "Fetching all bookmarks");
    let bookmarks = bookmark::get_by_user(&app_context.pool, claims.user_id).await?;
    info!(
        user_id = %claims.user_id,
        bookmark_count = %bookmarks.len(),
        "Retrieved bookmarks"
    );
    Ok(Json(Bookmarks { bookmarks }))
}

#[debug_handler]
async fn get_all_tags(
    claims: Claim,
    Extension(app_context): Extension<AppContext>,
) -> Result<Json<TagsWithCounters>> {
    debug!(user_id = %claims.user_id, "Fetching tag counts");
    let tags = bookmark::get_tag_count_by_user(&app_context.pool, claims.user_id).await?;
    let tags = tags
        .into_iter()
        .map(|(tag, count)| TagCount { tag, count })
        .collect::<Vec<_>>();
    info!(
        user_id = %claims.user_id,
        tag_count = %tags.len(),
        "Retrieved unique tags"
    );
    Ok(Json(TagsWithCounters { tags }))
}

#[debug_handler]
async fn get_bookmarks_by_tag(
    claims: Claim,
    Extension(app_context): Extension<AppContext>,
    Path(tag): Path<String>,
) -> Result<Json<Bookmarks>> {
    debug!(
        user_id = %claims.user_id,
        tag = %tag,
        "Fetching bookmarks with tag"
    );
    let bookmarks = bookmark::get_by_tag(&app_context.pool, claims.user_id, &tag).await?;
    info!(
        user_id = %claims.user_id,
        tag = %tag,
        bookmark_count = %bookmarks.len(),
        "Found bookmarks with tag"
    );
    Ok(Json(Bookmarks { bookmarks }))
}

#[debug_handler]
async fn get_bookmark(
    claims: Claim,
    Extension(app_context): Extension<AppContext>,
    Path(id): Path<String>,
) -> Result<Json<Bookmark>> {
    debug!(bookmark_id = %id, user_id = %claims.user_id, "Fetching bookmark");
    let maybe_bookmark =
        bookmark::get_with_user_data(&app_context.pool, claims.user_id, &id).await?;
    match maybe_bookmark {
        Some(bookmark) => {
            info!(
                bookmark_id = %bookmark.bookmark_id,
                url = %bookmark.url,
                "Bookmark retrieved"
            );
            Ok(Json(bookmark))
        }
        None => {
            info!(
                bookmark_id = %id,
                user_id = %claims.user_id,
                "Bookmark not found"
            );
            Err(Error::NotFound)
        }
    }
}

#[debug_handler]
async fn new_bookmark(
    claims: Claim,
    Extension(app_context): Extension<AppContext>,
    Json(input): Json<NewBookmark>,
) -> Result<(StatusCode, Json<BookmarkTask>)> {
    info!(
        user_id = %claims.user_id,
        url = %input.url,
        "Creating new bookmark"
    );

    // FIXME: move validation logic to a better place?
    let mut tags = input.tags.clone().unwrap_or_default();
    tags.retain(|t| !t.trim().is_empty());
    debug!(tags = ?tags, "Filtered tags");

    let response =
        bookmark_task::create(&app_context.pool, claims.user_id, input.url.clone(), tags).await?;

    if let Err(error) = app_context.tx_new_task.send(()) {
        error!(?error, "Failed to notify new task daemon");
    } else {
        debug!("Successfully notified task daemon of new bookmark task");
    }

    info!(
        task_id = %response.task_id,
        url = %input.url,
        "Bookmark task created"
    );
    Ok((StatusCode::CREATED, Json(response)))
}

#[debug_handler]
async fn set_tags(
    claims: Claim,
    Extension(app_context): Extension<AppContext>,
    Path(bookmark_id): Path<String>,
    Json(tags): Json<Tags>,
) -> Result<Json<Bookmark>> {
    info!(
        bookmark_id = %bookmark_id,
        user_id = %claims.user_id,
        new_tags = ?tags.tags,
        "Setting tags for bookmark"
    );
    let updated = bookmark::update_tags(
        &app_context.pool,
        claims.user_id,
        &bookmark_id,
        &TagOperation::Set(tags.tags),
    )
    .await?;
    info!(bookmark_id = %bookmark_id, "Tags successfully set");
    Ok(Json(updated))
}

#[debug_handler]
async fn append_tags(
    claims: Claim,
    Extension(app_context): Extension<AppContext>,
    Path(bookmark_id): Path<String>,
    Json(tags): Json<Tags>,
) -> Result<Json<Bookmark>> {
    info!(
        bookmark_id = %bookmark_id,
        user_id = %claims.user_id,
        additional_tags = ?tags.tags,
        "Appending tags to bookmark"
    );
    let updated = bookmark::update_tags(
        &app_context.pool,
        claims.user_id,
        &bookmark_id,
        &TagOperation::Append(tags.tags),
    )
    .await?;
    info!(bookmark_id = %bookmark_id, "Tags successfully appended");
    Ok(Json(updated))
}
