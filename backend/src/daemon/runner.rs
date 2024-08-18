use anyhow::{Context, Result};
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::Client as S3Client;
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

#[instrument(skip_all)]
pub async fn run(
    pool: &Pool<Postgres>,
    http: &HttpClient,
    s3_client: &S3Client,
    config: &Config,
) -> Result<()> {
    loop {
        let tasks: Vec<Task> = db::task::peek(pool, Utc::now()).await?;
        if tasks.is_empty() {
            let duration = TokioDuration::from_secs(10);
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
            tracing::info!(
                url = &task.url,
                task_id = format!("{}", &task.task_id),
                "Executing task"
            );
            match handle_task(pool, http, s3_client, config, &task).await {
                Ok(_) => {
                    tracing::info!("success");
                    db::task::update(pool, &task, TaskStatus::Done, None, None).await?;
                    tracing::info!(task_uuid = format!("{}", task.task_id), "Task executed")
                }
                Err(error) => {
                    if task.should_retry() {
                        tracing::info!("fail retry, reason = {:?}", &error);
                        let retry_value = task.retries.unwrap_or(0) + 1;
                        db::task::update(pool, &task, TaskStatus::Pending, Some(retry_value), None)
                            .await?;
                        tracing::warn!(
                            task_uuid = format!("{}", task.task_id),
                            retries = &task.retries,
                            "Task failed, retying, error info: {}",
                            error
                        )
                    } else {
                        tracing::info!("fail");
                        db::task::update(
                            pool,
                            &task,
                            TaskStatus::Fail,
                            None,
                            Some(format!("{error}")),
                        )
                        .await?;
                        tracing::error!(
                            task_uuid = format!("{}", task.task_id),
                            "Task failed, error info: {error}"
                        )
                    }
                }
            }
        }
    }
}

#[instrument(skip(pool, http, s3_client, config))]
async fn handle_task(
    pool: &Pool<Postgres>,
    http: &HttpClient,
    s3_client: &S3Client,
    config: &Config,
    task: &Task,
) -> Result<()> {
    let bookmark = crease_or_retrieve_bookmark(pool, http, s3_client, config, &task.url).await?;
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

#[instrument(skip(pool, http, s3_client, config))]
async fn crease_or_retrieve_bookmark(
    pool: &Pool<Postgres>,
    http: &HttpClient,
    s3_client: &S3Client,
    config: &Config,
    url: &str,
) -> Result<Bookmark> {
    match db::bookmark::get_by_url(pool, url).await? {
        Some(bookmark) => Ok(bookmark),
        None => {
            tracing::info!("Processing new bookmark for url={url}");
            let (bookmark, images) = processor::process_url(
                http,
                config.readability_url.clone(),
                url,
                config.s3_endpoint.clone(),
                &config.s3_bucket,
            )
            .await
            .with_context(|| format!("process_url: {url}"))?;
            save_static_content(s3_client, config, &bookmark, &images)
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
    s3_client: &S3Client,
    config: &Config,
    bookmark: &Bookmark,
    images: &[Image],
) -> Result<()> {
    tracing::info!(
        "Saving bookmark, id={}, images={}",
        &bookmark.bookmark_id,
        &bookmark.images.len()
    );
    for image in images.iter() {
        let key = format!("{}/{}", bookmark.bookmark_id, image.id);
        let put_response = s3_client
            .put_object()
            .bucket(&config.s3_bucket)
            .key(key)
            .content_type(&image.content_type)
            .body(ByteStream::from(image.bytes.to_vec()))
            .send()
            .await?;
        tracing::info!("Put response: {:?}", put_response);
    }
    Ok(())
}
