use anyhow::{Context, Result};
use shared::Bookmark;
use url::Url;

use super::{DAEMON_IDLE_SLEEP, TOKENIZER_WINDOW_OVERLAP, TOKENIZER_WINDOW_SIZE};
use crate::db::bookmark::{get_bookmarks_without_summary, get_text_content, update_summary};
use crate::db::PgPool;
use crate::{ollama, tokenizer};

const QUERY_LIMIT: usize = 10;

pub async fn run(
    pool: &PgPool,
    mut new_bookmark_rx: tokio::sync::watch::Receiver<()>,
    ollama_url: &Url,
    ollama_text_model: &str,
) -> Result<()> {
    let mut interval = tokio::time::interval(DAEMON_IDLE_SLEEP);
    loop {
        tokio::select! {
            _ = new_bookmark_rx.changed() => {
                tracing::info!("Notification receive, executing...");
                if let Err(error) = execute_step(pool, ollama_url, ollama_text_model).await {
                    tracing::error!(?error, "Fail to process tasks");
                }
            }
            _ = interval.tick() => {
                tracing::info!("{DAEMON_IDLE_SLEEP:?} passed, executing...");
                if let Err(error) = execute_step(pool, ollama_url, ollama_text_model).await {
                    tracing::error!(?error, "Fail to process tasks");
                }
            }
        }
    }
}

async fn execute_step(pool: &PgPool, ollama_url: &Url, ollama_text_model: &str) -> Result<()> {
    let tasks: Vec<Bookmark> = get_bookmarks_without_summary(pool, QUERY_LIMIT).await?;
    if tasks.is_empty() {
        tracing::info!("No new task");
        return Ok(());
    }
    tracing::info!("New tasks found: {}", tasks.len());
    for task in tasks {
        tracing::info!(?task, "Executing task");
        match handle_task(pool, ollama_url, ollama_text_model, &task).await {
            Ok(_) => {
                tracing::info!(id = task.bookmark_id, "Task executed")
            }
            Err(error) => {
                tracing::error!(?task, ?error, "Task failed");
            }
        }
    }
    Ok(())
}

async fn handle_task(
    pool: &PgPool,
    ollama_url: &Url,
    ollama_text_model: &str,
    bookmark: &Bookmark,
) -> Result<()> {
    let text_content = get_text_content(pool, bookmark.user_id, &bookmark.bookmark_id)
        .await?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Text content not found for bookmark_id: {}",
                &bookmark.bookmark_id
            )
        })
        .with_context(|| {
            format!(
                "Failed to fetch text_content from bookmark_id: {}",
                bookmark.bookmark_id
            )
        })?;

    let chunks = tokenizer::windowed_chunks(
        TOKENIZER_WINDOW_SIZE,
        TOKENIZER_WINDOW_OVERLAP,
        &text_content,
    )
    .with_context(|| {
        format!(
            "Failed to chunk text content for bookmark_id: {}",
            &bookmark.bookmark_id
        )
    })?;

    let mut summaries: Vec<String> = Vec::with_capacity(chunks.len());
    for chunk in chunks {
        let summary = ollama::summary(ollama_url, ollama_text_model, &chunk)
            .await
            .with_context(|| {
                format!(
                    "Failed to get summary from ollama from chunk: '{chunk}', bookmark_id: {}",
                    bookmark.bookmark_id
                )
            })?;
        summaries.push(summary);
    }

    let consolidated_summary =
        ollama::consolidate_summary(ollama_url, ollama_text_model, &summaries)
            .await
            .with_context(|| {
                format!(
                    "Failed to get consolidate summary from ollama, bookmark_id: {}",
                    bookmark.bookmark_id
                )
            })?;

    update_summary(
        pool,
        bookmark.user_id,
        &bookmark.bookmark_id,
        &consolidated_summary,
    )
    .await
    .with_context(|| {
        format!(
            "Failed to update bookmark with summary, bookmark_id: {}",
            bookmark.bookmark_id
        )
    })?;

    Ok(())
}
