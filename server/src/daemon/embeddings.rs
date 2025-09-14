use anyhow::{Context, Result};
use tracing::{debug, error, info, warn};
use url::Url;

use super::{DAEMON_IDLE_SLEEP, TOKENIZER_WINDOW_OVERLAP, TOKENIZER_WINDOW_SIZE};
use crate::db::chunks::{get_bookmarks_without_chunks, store_chunks_with_embeddings};
use crate::db::PgPool;
use crate::{ollama, tokenizer};

const QUERY_LIMIT: usize = 5; // Process fewer items at once to avoid overwhelming Ollama
const EMBEDDING_MODEL: &str = "nomic-embed-text:v1.5"; // Default embedding model

pub async fn run(
    pool: &PgPool,
    mut new_bookmark_rx: tokio::sync::watch::Receiver<()>,
    ollama_url: &Url,
    embedding_model: Option<&str>,
) -> Result<()> {
    let model = embedding_model.unwrap_or(EMBEDDING_MODEL);
    info!(model = %model, "Starting embeddings daemon");

    let mut interval = tokio::time::interval(DAEMON_IDLE_SLEEP);
    loop {
        // Process all available tasks continuously
        loop {
            match execute_step(pool, ollama_url, model).await {
                Ok(has_tasks) => {
                    if !has_tasks {
                        // No more tasks, exit inner loop
                        break;
                    }
                    // Continue processing if there were tasks
                }
                Err(error) => {
                    error!(?error, "Failed to process embedding tasks");
                    break; // Exit on error to avoid infinite loop
                }
            }
        }

        // Wait for notification or timeout when no tasks remain
        tokio::select! {
            _ = new_bookmark_rx.changed() => {
                info!("Notification received, checking for embedding tasks...");
                // Reset interval to avoid immediate timeout after notification
                interval.reset();
            }
            _ = interval.tick() => {
                debug!("{DAEMON_IDLE_SLEEP:?} passed, checking for embedding tasks...");
            }
        }
    }
}

async fn execute_step(pool: &PgPool, ollama_url: &Url, model: &str) -> Result<bool> {
    let bookmarks = get_bookmarks_without_chunks(pool, QUERY_LIMIT).await?;

    if bookmarks.is_empty() {
        debug!("No bookmarks without chunks found");
        return Ok(false);
    }

    info!(
        bookmark_count = bookmarks.len(),
        "Found bookmarks without chunks, processing..."
    );

    for (bookmark_id, user_id, text_content) in bookmarks {
        match process_bookmark_chunks(
            &bookmark_id,
            user_id,
            &text_content,
            pool,
            ollama_url,
            model,
        )
        .await
        {
            Ok(chunk_count) => {
                info!(
                    bookmark_id,
                    user_id = %user_id,
                    chunk_count,
                    "Successfully processed bookmark chunks"
                );
            }
            Err(error) => {
                warn!(
                    bookmark_id,
                    user_id = %user_id,
                    ?error,
                    "Failed to process bookmark chunks, will retry later"
                );
            }
        }
    }

    Ok(true) // There were tasks to process
}

async fn process_bookmark_chunks(
    bookmark_id: &str,
    user_id: uuid::Uuid,
    text_content: &str,
    pool: &PgPool,
    ollama_url: &Url,
    model: &str,
) -> Result<usize> {
    // Skip bookmarks with very little content
    if text_content.len() < 200 {
        debug!(
            bookmark_id,
            content_length = text_content.len(),
            "Skipping bookmark with insufficient content"
        );
        return Ok(0);
    }

    // Generate chunks using the existing tokenizer
    let chunks = tokenizer::windowed_chunks(
        TOKENIZER_WINDOW_SIZE,
        TOKENIZER_WINDOW_OVERLAP,
        text_content,
    )
    .context("Failed to create text chunks")?;

    if chunks.is_empty() {
        warn!(bookmark_id, "No chunks generated from text content");
        return Ok(0);
    }

    debug!(
        bookmark_id,
        chunk_count = chunks.len(),
        "Generated text chunks"
    );

    // Generate embeddings for each chunk
    let mut embeddings = Vec::new();
    for (index, chunk) in chunks.iter().enumerate() {
        match ollama::embeddings(ollama_url, model, chunk).await {
            Ok(embedding) => {
                debug!(
                    bookmark_id,
                    chunk_index = index,
                    embedding_dimensions = embedding.len(),
                    "Generated embedding for chunk"
                );
                embeddings.push(embedding);
            }
            Err(error) => {
                error!(
                    bookmark_id,
                    chunk_index = index,
                    ?error,
                    "Failed to generate embedding for chunk"
                );
                return Err(error);
            }
        }
    }

    // Store chunks with embeddings in database
    let stored_chunks =
        store_chunks_with_embeddings(pool, bookmark_id, user_id, chunks, embeddings)
            .await
            .context("Failed to store chunks with embeddings")?;

    info!(
        bookmark_id,
        user_id = %user_id,
        stored_chunk_count = stored_chunks.len(),
        "Successfully stored chunks with embeddings"
    );

    Ok(stored_chunks.len())
}
