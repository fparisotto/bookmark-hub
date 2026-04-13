use anyhow::{Context, Result};
use chrono::Utc;
use tracing::{debug, error, info};

use super::{
    ai_generation_backoff, AI_GENERATION_MAX_RETRIES, DAEMON_IDLE_SLEEP, TOKENIZER_WINDOW_OVERLAP,
    TOKENIZER_WINDOW_SIZE,
};
use crate::db::bookmark::{
    get_bookmarks_pending_summary_generation, get_text_content, mark_summary_generation_failure,
    update_summary, AiGenerationStatus, BookmarkGenerationCandidate,
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
        get_bookmarks_pending_summary_generation(pool, QUERY_LIMIT).await?;
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
                mark_summary_generation_failure(
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
        "Starting summary generation"
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
        "Retrieved text content for summarization"
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
        "Text chunked for summarization"
    );

    let mut summaries: Vec<String> = Vec::with_capacity(chunks.len());
    let total_chunks = chunks.len();

    for (chunk_idx, chunk) in chunks.iter().enumerate() {
        debug!(
            chunk_number = %(chunk_idx + 1),
            total_chunks = %total_chunks,
            bookmark_id = %bookmark.bookmark_id,
            chunk_length = %chunk.len(),
            "Summarizing chunk"
        );

        let start = std::time::Instant::now();
        let summary = llm::summary(client, chunk).await.with_context(|| {
            format!(
                "Failed to get summary for chunk {}/{}, bookmark_id: {}",
                chunk_idx + 1,
                total_chunks,
                bookmark.bookmark_id
            )
        })?;

        let elapsed = start.elapsed();
        debug!(
            chunk_number = %(chunk_idx + 1),
            total_chunks = %total_chunks,
            elapsed = ?elapsed,
            bookmark_id = %bookmark.bookmark_id,
            summary_length = %summary.len(),
            "Chunk summarized"
        );

        summaries.push(summary);
    }

    info!(
        chunk_summaries_count = %summaries.len(),
        bookmark_id = %bookmark.bookmark_id,
        "Consolidating chunk summaries"
    );
    debug!(
        summary_lengths = ?summaries.iter().map(|s| s.len()).collect::<Vec<_>>(),
        "Individual summary lengths"
    );

    let start = std::time::Instant::now();
    let consolidated_summary = llm::consolidate_summary(client, &summaries)
        .await
        .with_context(|| {
            format!(
                "Failed to consolidate summary, bookmark_id: {}",
                bookmark.bookmark_id
            )
        })?;

    let elapsed = start.elapsed();
    info!(
        elapsed = ?elapsed,
        bookmark_id = %bookmark.bookmark_id,
        final_length = %consolidated_summary.len(),
        "Summary consolidation completed"
    );
    debug!(
        summary_preview = %consolidated_summary.chars().take(100).collect::<String>(),
        "Final consolidated summary preview"
    );

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

    info!(
        bookmark_id = %bookmark.bookmark_id,
        final_summary_length = %consolidated_summary.len(),
        "Summary successfully updated"
    );

    Ok(())
}
