use std::time::Duration;

use anyhow::{Context, Result};
use chrono::Utc;
use reqwest::Client as HttpClient;
use uuid::Uuid;

use super::processor::Image;
use crate::daemon::processor;
use crate::db::{
    self,
    bookmark::Bookmark,
    task::{Task, TaskStatus},
    PgPool,
};
use crate::Config;

const DAEMON_IDLE_SLEEP: Duration = Duration::from_secs(300);

pub async fn run(
    pool: &PgPool,
    config: &Config,
    mut rx: tokio::sync::watch::Receiver<()>,
) -> Result<()> {
    let http: HttpClient = HttpClient::new();
    let mut interval = tokio::time::interval(DAEMON_IDLE_SLEEP);
    loop {
        tokio::select! {
            _ = rx.changed() => {
                tracing::info!("Notification receive, executing...");
                if let Err(error) = execute_step(pool, &http, config).await {
                    tracing::error!(?error, "Fail to process tasks");
                }
            }
            _ = interval.tick() => {
                tracing::info!("{DAEMON_IDLE_SLEEP:?} passed, executing...");
                if let Err(error) = execute_step(pool, &http, config).await {
                    tracing::error!(?error, "Fail to process tasks");
                }
            }
        }
    }
}

async fn execute_step(pool: &PgPool, http: &HttpClient, config: &Config) -> Result<()> {
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
                db::task::update(pool, task.clone(), TaskStatus::Done, None, None).await?;
                tracing::info!(task_uuid = format!("{}", task.task_id), "Task executed")
            }
            Err(error) => {
                if task.should_retry() {
                    let retry_value: i16 = task.retries.unwrap_or(0) + 1;
                    db::task::update(
                        pool,
                        task.clone(),
                        TaskStatus::Pending,
                        Some(retry_value),
                        None,
                    )
                    .await?;
                    tracing::warn!(?task, ?error, "Task failed, retying",)
                } else {
                    db::task::update(
                        pool,
                        task.clone(),
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

async fn handle_task(pool: &PgPool, http: &HttpClient, config: &Config, task: &Task) -> Result<()> {
    if db::bookmark::get_by_url_and_user_id(pool, &task.url, task.user_id)
        .await?
        .is_some()
    {
        tracing::info!(?task, "Duplicated bookmark");
        return Ok(());
    }

    tracing::info!("Processing new bookmark for url={}", &task.url);
    let output = processor::process_url(
        http,
        config.readability_url.clone(),
        config.ollama_url.clone(),
        config.ollama_model.clone(),
        &task.user_id,
        &task.url,
        &task.tags,
    )
    .await
    .with_context(|| format!("process_url: {}", &task.url))?;

    let bookmark = Bookmark {
        bookmark_id: output.bookmark_id,
        user_id: task.user_id,
        url: output.url,
        domain: output.domain,
        title: output.title,
        text_content: output.text_content,
        tags: output.tags,
        summary: output.summary,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    let bookmark_saved = db::bookmark::save(pool, &bookmark, output.embeddings.clone())
        .await
        .with_context(|| {
            format!(
                "save_bookmark_into_database: bookmark_id={}",
                &bookmark.bookmark_id
            )
        })?;

    save_static_content(
        config,
        &bookmark_saved,
        &output.images,
        &output.html,
        &task.user_id,
    )
    .await
    .with_context(|| {
        format!(
            "save_static_content: bookmark_id={}",
            &bookmark_saved.bookmark_id
        )
    })?;

    tracing::info!(
        url = task.url,
        bookmark_id = format!("{}", &bookmark_saved.bookmark_id),
        "Bookmark created",
    );
    Ok(())
}

async fn save_static_content(
    config: &Config,
    bookmark: &Bookmark,
    images: &[Image],
    content: &str,
    user_id: &Uuid,
) -> Result<()> {
    tracing::info!(
        "Saving bookmark, id={}, user_id={}",
        &bookmark.bookmark_id,
        user_id
    );
    let bookmark_dir = config
        .data_dir
        .join(user_id.to_string())
        .join(&bookmark.bookmark_id);

    if !bookmark_dir.exists() {
        tokio::fs::create_dir_all(&bookmark_dir).await?;
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
