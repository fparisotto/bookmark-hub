use std::collections::{BTreeSet, HashMap, HashSet};

use anyhow::{Context, Result};
use chrono::Utc;
use sha2::{Digest, Sha256};
use tracing::{debug, error, info};

use super::{
    ai_generation_backoff, AiDaemonSettings, AI_GENERATION_MAX_RETRIES, DAEMON_IDLE_SLEEP,
};
use crate::db::ai::{self, BookmarkAiChunk};
use crate::db::bookmark::{get_text_content, AiGenerationStatus, BookmarkGenerationCandidate};
use crate::db::PgPool;
use crate::llm::{self, LlmClient};
use crate::{tokenizer, TEXT_AI_PIPELINE_VERSION};

const QUERY_LIMIT: usize = 10;

pub async fn run(
    pool: &PgPool,
    mut new_bookmark_rx: tokio::sync::watch::Receiver<()>,
    client: &LlmClient,
    settings: &AiDaemonSettings,
) -> Result<()> {
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
                    error!(?error, "Failed to process unified text AI tasks");
                    break;
                }
            }
        }

        tokio::select! {
            _ = new_bookmark_rx.changed() => {
                info!("Notification received, checking unified text AI tasks...");
                interval.reset();
            }
            _ = interval.tick() => {
                info!("{DAEMON_IDLE_SLEEP:?} passed, checking unified text AI tasks...");
            }
        }
    }
}

async fn execute_step(
    pool: &PgPool,
    client: &LlmClient,
    settings: &AiDaemonSettings,
) -> Result<bool> {
    let tasks = ai::claim_bookmarks_pending_text_ai(
        pool,
        QUERY_LIMIT,
        Utc::now(),
        settings.text_claim_window,
    )
    .await?;
    if tasks.is_empty() {
        info!("No unified text AI task");
        return Ok(false);
    }

    info!(task_count = tasks.len(), "Claimed unified text AI tasks");
    for candidate in tasks {
        let task = &candidate.bookmark;
        match handle_task(pool, client, settings, &candidate).await {
            Ok(()) => {
                info!(bookmark_id = %task.bookmark_id, "Unified text AI task completed");
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
                ai::mark_text_ai_failure(
                    pool,
                    task.user_id,
                    &task.bookmark_id,
                    status,
                    attempts,
                    next_attempt_at,
                    &format!("{error:#}"),
                )
                .await?;
                if is_terminal {
                    error!(
                        bookmark_id = %task.bookmark_id,
                        ?error,
                        attempts,
                        "Unified text AI task failed permanently"
                    );
                } else {
                    error!(
                        bookmark_id = %task.bookmark_id,
                        ?error,
                        attempts,
                        next_attempt_at = %next_attempt_at,
                        "Unified text AI task failed, scheduled retry"
                    );
                }
            }
        }
    }

    Ok(true)
}

fn hash_chunk(chunk: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(chunk.as_bytes());
    hex::encode(hasher.finalize())
}

fn normalize_tags(tags: Vec<String>) -> Option<Vec<String>> {
    let mut seen = HashSet::new();
    let normalized = tags
        .into_iter()
        .map(|tag| tag.trim().to_lowercase())
        .filter(|tag| !tag.is_empty() && seen.insert(tag.clone()))
        .collect::<Vec<_>>();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

async fn handle_task(
    pool: &PgPool,
    client: &LlmClient,
    settings: &AiDaemonSettings,
    candidate: &BookmarkGenerationCandidate,
) -> Result<()> {
    let bookmark = &candidate.bookmark;
    let text_content = get_text_content(pool, bookmark.user_id, &bookmark.bookmark_id)
        .await?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Text content not found for bookmark_id={}",
                bookmark.bookmark_id
            )
        })?;

    let chunks = tokenizer::windowed_chunks(
        settings.text_chunk_size,
        settings.text_chunk_overlap,
        &text_content,
    )
    .with_context(|| {
        format!(
            "Failed to chunk text for bookmark_id={}",
            bookmark.bookmark_id
        )
    })?;

    let chunk_hashes = chunks
        .iter()
        .map(|chunk| hash_chunk(chunk))
        .collect::<Vec<_>>();
    let existing_rows =
        ai::list_bookmark_ai_chunks(pool, bookmark.user_id, &bookmark.bookmark_id).await?;

    let layout_changed = existing_rows.len() != chunks.len()
        || existing_rows.iter().any(|row| {
            let idx = row.chunk_index as usize;
            idx >= chunk_hashes.len()
                || row.pipeline_version != TEXT_AI_PIPELINE_VERSION
                || row.chunk_hash != chunk_hashes[idx]
        });

    if layout_changed && !existing_rows.is_empty() {
        ai::delete_bookmark_ai_chunks(pool, bookmark.user_id, &bookmark.bookmark_id).await?;
    }

    let existing_map: HashMap<i32, BookmarkAiChunk> = if layout_changed {
        HashMap::new()
    } else {
        existing_rows
            .into_iter()
            .map(|row| (row.chunk_index, row))
            .collect()
    };

    let mut analyses = Vec::with_capacity(chunks.len());
    for (index, chunk) in chunks.iter().enumerate() {
        let chunk_index = index as i32;
        let analysis = if let Some(existing) = existing_map.get(&chunk_index) {
            existing.clone()
        } else {
            let response = llm::analyze_chunk(client, chunk).await.with_context(|| {
                format!(
                    "Failed to analyze chunk {}/{} for bookmark_id={}",
                    index + 1,
                    chunks.len(),
                    bookmark.bookmark_id
                )
            })?;
            let stored = BookmarkAiChunk {
                bookmark_id: bookmark.bookmark_id.clone(),
                user_id: bookmark.user_id,
                chunk_index,
                chunk_hash: chunk_hashes[index].clone(),
                pipeline_version: TEXT_AI_PIPELINE_VERSION,
                summary: response.summary,
                tags: response.tags,
            };
            ai::upsert_bookmark_ai_chunk(pool, &stored).await?;
            ai::refresh_text_ai_claim(
                pool,
                bookmark.user_id,
                &bookmark.bookmark_id,
                Utc::now() + settings.text_claim_window,
            )
            .await?;
            stored
        };
        analyses.push(analysis);
    }

    let summary = if candidate.needs_summary && !analyses.is_empty() {
        let summaries = analyses
            .iter()
            .map(|analysis| analysis.summary.clone())
            .collect::<Vec<_>>();
        let summary = llm::consolidate_summary(client, &summaries)
            .await
            .with_context(|| {
                format!(
                    "Failed to consolidate summary for bookmark_id={}",
                    bookmark.bookmark_id
                )
            })?;
        let summary = summary.trim().to_string();
        if summary.is_empty() {
            None
        } else {
            Some(summary)
        }
    } else {
        bookmark.summary.clone()
    };

    let tags = if candidate.needs_tags && !analyses.is_empty() {
        let raw_tags = analyses
            .iter()
            .flat_map(|analysis| analysis.tags.iter().cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let tags = llm::consolidate_tags(client, raw_tags)
            .await
            .with_context(|| {
                format!(
                    "Failed to consolidate tags for bookmark_id={}",
                    bookmark.bookmark_id
                )
            })?;
        normalize_tags(tags)
    } else {
        bookmark.tags.clone()
    };

    ai::refresh_text_ai_claim(
        pool,
        bookmark.user_id,
        &bookmark.bookmark_id,
        Utc::now() + settings.text_claim_window,
    )
    .await?;
    ai::complete_text_ai_outputs(
        pool,
        bookmark.user_id,
        &bookmark.bookmark_id,
        summary.as_deref(),
        tags.as_deref(),
        TEXT_AI_PIPELINE_VERSION,
    )
    .await?;

    debug!(
        bookmark_id = %bookmark.bookmark_id,
        summary_present = summary.is_some(),
        tag_count = tags.as_ref().map(|values| values.len()).unwrap_or(0),
        "Unified text AI outputs persisted"
    );
    Ok(())
}
