use anyhow::{Context, Result};
use chrono::Utc;
use reqwest::Client as HttpClient;
use sqlx::{Pool, Postgres};
use tokio::time::{sleep, Duration as TokioDuration};
use tracing::instrument;

use super::processor::Image;
use crate::daemon::processor;
use crate::db::{
    self,
    bookmark::Bookmark,
    task::{Task, TaskStatus},
};
use crate::Config;

const DAEMON_IDLE_SLEEP: u64 = 10;

#[instrument(skip_all)]
pub async fn run(pool: &Pool<Postgres>, http: &HttpClient, config: &Config) -> Result<()> {
    // TODO wake up with notification
    loop {
        let tasks: Vec<Task> = db::task::peek(pool, Utc::now()).await?;
        if tasks.is_empty() {
            let duration = TokioDuration::from_secs(DAEMON_IDLE_SLEEP);
            tracing::debug!(
                "No new task, going to sleep for {} seconds",
                &duration.as_secs()
            );
            sleep(duration).await;
            continue;
        } else {
            tracing::info!("New tasks found {}", tasks.len());
        }
        for task in tasks {
            tracing::info!(?task, "Executing task");
            match handle_task(pool, http, config, &task).await {
                Ok(_) => {
                    db::task::update(pool, &task, TaskStatus::Done, None, None).await?;
                    tracing::info!(task_uuid = format!("{}", task.task_id), "Task executed")
                }
                Err(error) => {
                    if task.should_retry() {
                        let retry_value: i16 = task.retries.unwrap_or(0) + 1;
                        db::task::update(pool, &task, TaskStatus::Pending, Some(retry_value), None)
                            .await?;
                        tracing::warn!(?task, ?error, "Task failed, retying",)
                    } else {
                        db::task::update(
                            pool,
                            &task,
                            TaskStatus::Fail,
                            None,
                            Some(format!("{error}")),
                        )
                        .await?;
                        tracing::error!(?task, ?error, "Task failed");
                    }
                }
            }
        }
    }
}

#[instrument(skip(pool, http, config))]
async fn handle_task(
    pool: &Pool<Postgres>,
    http: &HttpClient,
    config: &Config,
    task: &Task,
) -> Result<()> {
    let bookmark = crease_or_retrieve_bookmark(pool, http, config, &task.url).await?;
    let uuid = db::bookmark::upsert_user_bookmark(
        pool,
        &bookmark.bookmark_id,
        task.user_id,
        task.tags.clone(),
    )
    .await?;
    tracing::info!(
        user_id = format!("{}", task.user_id),
        bookmark_user_id = format!("{uuid}"),
        bookmark_id = &bookmark.bookmark_id,
        "Creating a new bookmark and bound with user",
    );
    Ok(())
}

#[instrument(skip(pool, http, config))]
async fn crease_or_retrieve_bookmark(
    pool: &Pool<Postgres>,
    http: &HttpClient,
    config: &Config,
    url: &str,
) -> Result<Bookmark> {
    match db::bookmark::get_by_url(pool, url).await? {
        Some(bookmark) => Ok(bookmark),
        None => {
            tracing::info!("Processing new bookmark for url={url}");
            let (bookmark, images) =
                processor::process_url(http, config.readability_url.clone(), url)
                    .await
                    .with_context(|| format!("process_url: {url}"))?;
            save_static_content(config, &bookmark, &images)
                .await
                .with_context(|| {
                    format!("save_static_content: bookmark_id={}", &bookmark.bookmark_id)
                })?;
            db::bookmark::save(pool, &bookmark).await.with_context(|| {
                format!(
                    "save_bookmark_into_database: bookmark_id={}",
                    &bookmark.bookmark_id
                )
            })?;
            tracing::info!(
                url = url,
                bookmark_id = format!("{}", &bookmark.bookmark_id),
                "Bookmark created",
            );
            Ok(bookmark)
        }
    }
}

async fn save_static_content(config: &Config, bookmark: &Bookmark, images: &[Image]) -> Result<()> {
    tracing::info!(
        "Saving bookmark, id={}, images={}",
        &bookmark.bookmark_id,
        &bookmark.images.len()
    );
    let image_dir = config.data_dir.join(&bookmark.bookmark_id);
    if !image_dir.exists() {
        tokio::fs::create_dir(&image_dir).await?;
    }
    for image in images.iter() {
        let image_path = image_dir.join(&image.id);
        if image_path.exists() {
            tracing::info!(?image_path, "Image is already there");
            continue;
        }
        tokio::fs::write(&image_path, &image.bytes).await?;
        tracing::info!(?image_path, "Image file saved");
    }
    Ok(())
}
