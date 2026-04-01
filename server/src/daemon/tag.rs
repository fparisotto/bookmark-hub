use std::collections::BTreeSet;

use anyhow::{Context, Result};
use chrono::Utc;
use tracing::{debug, error, info};

use super::{
    ai_generation_backoff, AI_GENERATION_MAX_RETRIES, DAEMON_IDLE_SLEEP, TOKENIZER_WINDOW_OVERLAP,
    TOKENIZER_WINDOW_SIZE,
};
use crate::db::bookmark::{
    get_bookmarks_pending_tag_generation, get_text_content, mark_tag_generation_failure,
    update_tags, AiGenerationStatus, BookmarkGenerationCandidate,
};
use crate::db::PgPool;
use crate::llm::{self, LlmClient};
use crate::tokenizer;

const QUERY_LIMIT: usize = 10;

pub async fn run(
    pool: &PgPool,
    mut new_bookmark_rx: tokio::sync::watch::Receiver<()>,
    client: &LlmClient,
) -> Result<()> {
    let mut interval = tokio::time::interval(DAEMON_IDLE_SLEEP);
    loop {
        // Process all available tasks continuously
        loop {
            match execute_step(pool, client).await {
                Ok(has_tasks) => {
                    if !has_tasks {
                        // No more tasks, exit inner loop
                        break;
                    }
                    // Continue processing if there were tasks
                }
                Err(error) => {
                    error!(?error, "Fail to process tasks");
                    break; // Exit on error to avoid infinite loop
                }
            }
        }

        // Wait for notification or timeout when no tasks remain
        tokio::select! {
            _ = new_bookmark_rx.changed() => {
                info!("Notification received, checking for tasks...");
                // Reset interval to avoid immediate timeout after notification
                interval.reset();
            }
            _ = interval.tick() => {
                info!("{DAEMON_IDLE_SLEEP:?} passed, checking for tasks...");
            }
        }
    }
}

async fn execute_step(pool: &PgPool, client: &LlmClient) -> Result<bool> {
    let tasks: Vec<BookmarkGenerationCandidate> =
        get_bookmarks_pending_tag_generation(pool, QUERY_LIMIT, Utc::now()).await?;
    if tasks.is_empty() {
        info!("No new task");
        return Ok(false);
    }
    info!("New tasks found: {}", tasks.len());
    let mut any_progress = false;
    for candidate in tasks {
        let task = &candidate.bookmark;
        info!(?task, attempts = candidate.attempts, "Executing task");
        match handle_task(pool, client, task).await {
            Ok(_) => {
                any_progress = true;
                info!(id = task.bookmark_id, "Task executed")
            }
            Err(error) => {
                let attempts = candidate.attempts + 1;
                let is_terminal = attempts >= AI_GENERATION_MAX_RETRIES;
                let status = if is_terminal {
                    AiGenerationStatus::Fail
                } else {
                    AiGenerationStatus::Pending
                };
                let next_attempt_at = if is_terminal {
                    Utc::now()
                } else {
                    Utc::now() + ai_generation_backoff(attempts)
                };
                mark_tag_generation_failure(
                    pool,
                    task.user_id,
                    &task.bookmark_id,
                    status,
                    attempts,
                    next_attempt_at,
                    &format!("{error:#}"),
                )
                .await?;
                any_progress = true;
                if is_terminal {
                    error!(
                        ?task,
                        ?error,
                        attempts,
                        "Task failed permanently after max retries"
                    );
                } else {
                    error!(
                        ?task,
                        ?error,
                        attempts,
                        next_attempt_at = %next_attempt_at,
                        "Task failed, scheduled retry"
                    );
                }
            }
        }
    }
    Ok(any_progress)
}

async fn handle_task(pool: &PgPool, client: &LlmClient, bookmark: &shared::Bookmark) -> Result<()> {
    info!(
        bookmark_id = %bookmark.bookmark_id,
        title = %bookmark.title,
        user_id = %bookmark.user_id,
        "Starting tag generation"
    );

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

    debug!(
        bookmark_id = %bookmark.bookmark_id,
        content_length = %text_content.len(),
        "Retrieved text content for tagging"
    );

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

    info!(
        bookmark_id = %bookmark.bookmark_id,
        chunks_count = %chunks.len(),
        window_size = %TOKENIZER_WINDOW_SIZE,
        overlap = %TOKENIZER_WINDOW_OVERLAP,
        "Text chunked for processing"
    );

    let mut tags: BTreeSet<String> = BTreeSet::new();
    let total_chunks = chunks.len();

    for (chunk_idx, chunk) in chunks.iter().enumerate() {
        debug!(
            chunk_number = %(chunk_idx + 1),
            total_chunks = %total_chunks,
            bookmark_id = %bookmark.bookmark_id,
            chunk_length = %chunk.len(),
            "Processing chunk"
        );

        let start = std::time::Instant::now();
        let response = llm::tags(client, chunk).await.with_context(|| {
            format!(
                "Failed to get tags for chunk {}/{}, bookmark_id: {}",
                chunk_idx + 1,
                total_chunks,
                bookmark.bookmark_id
            )
        })?;

        let elapsed = start.elapsed();
        debug!(
            chunk_number = %(chunk_idx + 1),
            total_chunks = %total_chunks,
            tag_count = %response.len(),
            elapsed = ?elapsed,
            bookmark_id = %bookmark.bookmark_id,
            "Tags response for chunk"
        );

        let tags_before = tags.len();
        for tag in response {
            tags.insert(tag);
        }
        let new_tags = tags.len() - tags_before;

        if new_tags > 0 {
            debug!(
                new_tags = %new_tags,
                chunk_number = %(chunk_idx + 1),
                total_chunks = %total_chunks,
                total_unique_tags = %tags.len(),
                "Added new tags from chunk"
            );
        }
    }

    info!(
        raw_tag_count = %tags.len(),
        bookmark_id = %bookmark.bookmark_id,
        "Consolidating raw tags"
    );
    debug!(
        raw_tags = ?tags.iter().collect::<Vec<_>>(),
        "Raw tags before consolidation"
    );

    let start = std::time::Instant::now();
    let consolidated_tags =
        llm::consolidate_tags(client, tags.iter().map(|e| e.to_owned()).collect())
            .await
            .with_context(|| {
                format!(
                    "Failed to consolidate tags, bookmark_id: {}",
                    bookmark.bookmark_id
                )
            })?;

    let elapsed = start.elapsed();
    info!(
        elapsed = ?elapsed,
        bookmark_id = %bookmark.bookmark_id,
        raw_tag_count = %tags.len(),
        final_tag_count = %consolidated_tags.len(),
        "Tag consolidation completed"
    );
    debug!(final_tags = ?consolidated_tags, "Final consolidated tags");

    update_tags(
        pool,
        bookmark.user_id,
        &bookmark.bookmark_id,
        &shared::TagOperation::Set(consolidated_tags.clone()),
    )
    .await
    .with_context(|| {
        format!(
            "Failed to update bookmark with tags, bookmark_id: {}",
            bookmark.bookmark_id
        )
    })?;

    info!(
        bookmark_id = %bookmark.bookmark_id,
        final_tag_count = %consolidated_tags.len(),
        "Tags successfully updated"
    );

    Ok(())
}
