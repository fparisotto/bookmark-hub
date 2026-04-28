use anyhow::{Context, Result};
use chrono::Utc;
use tracing::{debug, error, info, warn};

use super::{
    ai_generation_backoff, AiDaemonSettings, AI_GENERATION_MAX_RETRIES, DAEMON_IDLE_SLEEP,
};
use crate::db::ai::{self, EmbeddingGenerationCandidate};
use crate::db::bookmark::AiGenerationStatus;
use crate::db::chunks::store_chunks_with_embeddings;
use crate::db::PgPool;
use crate::llm::{self, LlmClient};
use crate::{tokenizer, EMBEDDING_PIPELINE_VERSION};

const QUERY_LIMIT: usize = 5;

pub async fn run(
    pool: &PgPool,
    mut new_bookmark_rx: tokio::sync::watch::Receiver<()>,
    client: &LlmClient,
    settings: &AiDaemonSettings,
) -> Result<()> {
    info!(
        model = %client.embedding_model,
        ndims = %client.embedding_ndims,
        "Starting embeddings daemon"
    );

    let mut interval = tokio::time::interval(DAEMON_IDLE_SLEEP);
    loop {
        loop {
            match execute_step(pool, client, settings).await {
                Ok(has_tasks) => {
                    if !has_tasks {
                        break;
                    }
                }
                Err(error) => {
                    error!(?error, "Failed to process embedding tasks");
                    break;
                }
            }
        }

        tokio::select! {
            _ = new_bookmark_rx.changed() => {
                info!("Notification received, checking for embedding tasks...");
                interval.reset();
            }
            _ = interval.tick() => {
                debug!("{DAEMON_IDLE_SLEEP:?} passed, checking for embedding tasks...");
            }
        }
    }
}

async fn execute_step(
    pool: &PgPool,
    client: &LlmClient,
    settings: &AiDaemonSettings,
) -> Result<bool> {
    let bookmarks: Vec<EmbeddingGenerationCandidate> = ai::claim_bookmarks_pending_embeddings(
        pool,
        QUERY_LIMIT,
        Utc::now(),
        settings.embed_claim_window,
    )
    .await?;

    if bookmarks.is_empty() {
        debug!("No bookmarks pending embeddings");
        return Ok(false);
    }

    info!(bookmark_count = bookmarks.len(), "Claimed embedding tasks");

    for candidate in bookmarks {
        match process_bookmark_chunks(&candidate, pool, client, settings).await {
            Ok(chunk_count) => {
                info!(
                    bookmark_id = %candidate.bookmark_id,
                    user_id = %candidate.user_id,
                    chunk_count,
                    "Successfully processed embeddings"
                );
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
                ai::mark_embedding_failure(
                    pool,
                    candidate.user_id,
                    &candidate.bookmark_id,
                    status,
                    attempts,
                    next_attempt_at,
                    &format!("{error:#}"),
                )
                .await?;
                if is_terminal {
                    warn!(
                        bookmark_id = %candidate.bookmark_id,
                        ?error,
                        attempts,
                        "Embedding task failed permanently"
                    );
                } else {
                    warn!(
                        bookmark_id = %candidate.bookmark_id,
                        ?error,
                        attempts,
                        next_attempt_at = %next_attempt_at,
                        "Embedding task failed, scheduled retry"
                    );
                }
            }
        }
    }

    Ok(true)
}

async fn process_bookmark_chunks(
    candidate: &EmbeddingGenerationCandidate,
    pool: &PgPool,
    client: &LlmClient,
    settings: &AiDaemonSettings,
) -> Result<usize> {
    if candidate.text_content.len() < 200 {
        ai::mark_embedding_done_without_chunks(
            pool,
            candidate.user_id,
            &candidate.bookmark_id,
            EMBEDDING_PIPELINE_VERSION,
        )
        .await?;
        return Ok(0);
    }

    let chunks = tokenizer::windowed_chunks(
        settings.embed_chunk_size,
        settings.embed_chunk_overlap,
        &candidate.text_content,
    )
    .context("Failed to create embedding text chunks")?;

    if chunks.is_empty() {
        ai::mark_embedding_done_without_chunks(
            pool,
            candidate.user_id,
            &candidate.bookmark_id,
            EMBEDDING_PIPELINE_VERSION,
        )
        .await?;
        return Ok(0);
    }

    let mut embeddings = Vec::with_capacity(chunks.len());
    for (index, chunk) in chunks.iter().enumerate() {
        let embedding = llm::embeddings_background(client, chunk)
            .await
            .with_context(|| {
                format!(
                    "Failed to generate embedding for chunk {}/{} of bookmark_id={}",
                    index + 1,
                    chunks.len(),
                    candidate.bookmark_id
                )
            })?;
        embeddings.push(embedding);
        ai::refresh_embedding_claim(
            pool,
            candidate.user_id,
            &candidate.bookmark_id,
            Utc::now() + settings.embed_claim_window,
        )
        .await?;
    }

    let stored_chunks = store_chunks_with_embeddings(
        pool,
        &candidate.bookmark_id,
        candidate.user_id,
        chunks,
        embeddings,
    )
    .await
    .context("Failed to store chunks with embeddings")?;

    ai::mark_embedding_success(
        pool,
        candidate.user_id,
        &candidate.bookmark_id,
        EMBEDDING_PIPELINE_VERSION,
    )
    .await?;

    Ok(stored_chunks.len())
}
