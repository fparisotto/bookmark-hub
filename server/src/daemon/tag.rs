use std::collections::BTreeSet;

use anyhow::{Context, Result};
use shared::Bookmark;
use url::Url;

use super::{DAEMON_IDLE_SLEEP, TOKENIZER_WINDOW_OVERLAP, TOKENIZER_WINDOW_SIZE};
use crate::db::bookmark::{get_text_content, get_untagged_bookmarks, update_tags};
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
        // Process all available tasks continuously
        loop {
            match execute_step(pool, ollama_url, ollama_text_model).await {
                Ok(has_tasks) => {
                    if !has_tasks {
                        // No more tasks, exit inner loop
                        break;
                    }
                    // Continue processing if there were tasks
                }
                Err(error) => {
                    tracing::error!(?error, "Fail to process tasks");
                    break; // Exit on error to avoid infinite loop
                }
            }
        }

        // Wait for notification or timeout when no tasks remain
        tokio::select! {
            _ = new_bookmark_rx.changed() => {
                tracing::info!("Notification received, checking for tasks...");
                // Reset interval to avoid immediate timeout after notification
                interval.reset();
            }
            _ = interval.tick() => {
                tracing::info!("{DAEMON_IDLE_SLEEP:?} passed, checking for tasks...");
            }
        }
    }
}

async fn execute_step(pool: &PgPool, ollama_url: &Url, ollama_text_model: &str) -> Result<bool> {
    let tasks: Vec<Bookmark> = get_untagged_bookmarks(pool, QUERY_LIMIT).await?;
    if tasks.is_empty() {
        tracing::info!("No new task");
        return Ok(false);
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
    Ok(true)
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

    let mut tags: BTreeSet<String> = BTreeSet::new();
    for chunk in chunks {
        let response = ollama::tags(ollama_url, ollama_text_model, &chunk)
            .await
            .with_context(|| {
                format!(
                    "Failed to get tags from ollama from chunk: '{chunk}', bookmark_id: {}",
                    bookmark.bookmark_id
                )
            })?;
        for tag in response {
            tags.insert(tag);
        }
    }

    let consolidated_tags = ollama::consolidate_tags(
        ollama_url,
        ollama_text_model,
        tags.iter().map(|e| e.to_owned()).collect(),
    )
    .await
    .with_context(|| {
        format!(
            "Failed to get consolidate tags from ollama, bookmark_id: {}",
            bookmark.bookmark_id
        )
    })?;

    update_tags(
        pool,
        bookmark.user_id,
        &bookmark.bookmark_id,
        &shared::TagOperation::Set(consolidated_tags),
    )
    .await
    .with_context(|| {
        format!(
            "Failed to update bookmark with tags, bookmark_id: {}",
            bookmark.bookmark_id
        )
    })?;

    Ok(())
}
