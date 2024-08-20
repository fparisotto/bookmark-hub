use std::time::Duration;

use anyhow::{Context, Result};
use chrono::Utc;
use reqwest::Client as HttpClient;
use sqlx::{Pool, Postgres};
use tracing::instrument;

use super::processor::Image;
use crate::daemon::processor;
use crate::db::{
    self,
    bookmark::Bookmark,
    task::{Task, TaskStatus},
};
use crate::Config;

const DAEMON_IDLE_SLEEP: u64 = 60;

#[instrument(skip_all)]
pub async fn run(
    pool: &Pool<Postgres>,
    config: &Config,
    mut rx: tokio::sync::watch::Receiver<()>,
) -> Result<()> {
    let http: HttpClient = HttpClient::new();
    let mut interval = tokio::time::interval(Duration::from_secs(DAEMON_IDLE_SLEEP));
    loop {
        tokio::select! {
            _ = rx.changed() => {
                tracing::info!("Notification receive, executing...");
                if let Err(error) = execute_step(pool, &http, config).await {
                    tracing::error!(?error, "Fail to process tasks");
                }
            }
            _ = interval.tick() => {
                tracing::info!("Seconds {DAEMON_IDLE_SLEEP} passed, executing...");
                if let Err(error) = execute_step(pool, &http, config).await {
                    tracing::error!(?error, "Fail to process tasks");
                }
            }
        }
    }
}

async fn execute_step(pool: &Pool<Postgres>, http: &HttpClient, config: &Config) -> Result<()> {
    let tasks: Vec<Task> = db::task::peek(pool, Utc::now()).await?;
    if tasks.is_empty() {
        tracing::info!("No new task");
        return Ok(());
    }
    tracing::info!("New tasks found: {}", tasks.len());
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
    Ok(())
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
            let (bookmark, images, content) =
                processor::process_url(http, config.readability_url.clone(), url)
                    .await
                    .with_context(|| format!("process_url: {url}"))?;
            save_static_content(config, &bookmark, &images, &content)
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

async fn save_static_content(
    config: &Config,
    bookmark: &Bookmark,
    images: &[Image],
    content: &str,
) -> Result<()> {
    tracing::info!("Saving bookmark, id={}", &bookmark.bookmark_id,);
    let bookmark_dir = config.data_dir.join(&bookmark.bookmark_id);
    if !bookmark_dir.exists() {
        tokio::fs::create_dir(&bookmark_dir).await?;
    }
    let index = bookmark_dir.join("index.html");
    tokio::fs::write(&index, content).await?;
    for image in images.iter() {
        let image_path = bookmark_dir.join(&image.id);
        if image_path.exists() {
            tracing::info!(?image_path, "Image is already there");
            continue;
        }
        tokio::fs::write(&image_path, &image.bytes).await?;
        tracing::info!(?image_path, "Image file saved");
    }
    Ok(())
}
