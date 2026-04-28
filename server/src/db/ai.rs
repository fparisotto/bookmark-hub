use chrono::{DateTime, Duration as ChronoDuration, Utc};
use postgres_from_row::FromRow;
use shared::Bookmark;
use tracing::{debug, info};
use uuid::Uuid;

use super::PgPool;
use crate::db::bookmark::{AiGenerationStatus, BookmarkGenerationCandidate};
use crate::error::{Error, Result};

const MAX_FAILURE_REASON_LEN: usize = 2048;

#[derive(Debug, Clone)]
pub struct EmbeddingGenerationCandidate {
    pub bookmark_id: String,
    pub user_id: Uuid,
    pub text_content: String,
    pub attempts: i16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookmarkAiChunk {
    pub bookmark_id: String,
    pub user_id: Uuid,
    pub chunk_index: i32,
    pub chunk_hash: String,
    pub pipeline_version: i32,
    pub summary: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, FromRow)]
struct RowBookmark {
    bookmark_id: String,
    user_id: Uuid,
    url: String,
    domain: String,
    title: String,
    tags: Option<Vec<String>>,
    summary: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: Option<DateTime<Utc>>,
}

impl From<RowBookmark> for Bookmark {
    fn from(value: RowBookmark) -> Self {
        Self {
            bookmark_id: value.bookmark_id,
            user_id: value.user_id,
            url: value.url,
            domain: value.domain,
            title: value.title,
            tags: value.tags,
            summary: value.summary,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

fn truncate_fail_reason(fail_reason: &str) -> String {
    if fail_reason.chars().count() <= MAX_FAILURE_REASON_LEN {
        return fail_reason.to_string();
    }

    fail_reason.chars().take(MAX_FAILURE_REASON_LEN).collect()
}

pub async fn claim_bookmarks_pending_text_ai(
    pool: &PgPool,
    limit: usize,
    now: DateTime<Utc>,
    claim_window: ChronoDuration,
) -> Result<Vec<BookmarkGenerationCandidate>> {
    const QUERY: &str = r#"
        SELECT *
        FROM bookmark
        WHERE text_ai_status = 'pending'
          AND text_ai_next_attempt_at <= $1
          AND (summary_status = 'pending' OR tag_status = 'pending')
        ORDER BY text_ai_next_attempt_at ASC, created_at ASC
        FOR UPDATE SKIP LOCKED
        LIMIT $2;
    "#;

    let mut client = pool.get().await?;
    let tx = client.transaction().await?;
    let rows = tx.query(QUERY, &[&now, &(limit as i64)]).await?;
    let candidates = rows
        .iter()
        .map(|row| {
            let bookmark = RowBookmark::try_from_row(row)
                .map(Bookmark::from)
                .map_err(Error::from)?;
            let attempts: i16 = row.try_get("text_ai_attempts").map_err(Error::from)?;
            let summary_status: AiGenerationStatus =
                row.try_get("summary_status").map_err(Error::from)?;
            let tag_status: AiGenerationStatus = row.try_get("tag_status").map_err(Error::from)?;
            Ok(BookmarkGenerationCandidate {
                bookmark,
                attempts,
                needs_summary: summary_status == AiGenerationStatus::Pending,
                needs_tags: tag_status == AiGenerationStatus::Pending,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let claimed_until = now + claim_window;
    for candidate in &candidates {
        tx.execute(
            "UPDATE bookmark
             SET text_ai_next_attempt_at = $1
             WHERE bookmark_id = $2 AND user_id = $3",
            &[
                &claimed_until,
                &candidate.bookmark.bookmark_id,
                &candidate.bookmark.user_id,
            ],
        )
        .await?;
    }
    tx.commit().await?;

    if !candidates.is_empty() {
        info!(
            task_count = candidates.len(),
            claimed_until = %claimed_until,
            "Claimed bookmarks for unified text AI processing"
        );
    }
    Ok(candidates)
}

pub async fn claim_bookmarks_pending_embeddings(
    pool: &PgPool,
    limit: usize,
    now: DateTime<Utc>,
    claim_window: ChronoDuration,
) -> Result<Vec<EmbeddingGenerationCandidate>> {
    const QUERY: &str = r#"
        SELECT bookmark_id, user_id, text_content, embedding_attempts
        FROM bookmark
        WHERE embedding_status = 'pending'
          AND embedding_next_attempt_at <= $1
        ORDER BY embedding_next_attempt_at ASC, created_at ASC
        FOR UPDATE SKIP LOCKED
        LIMIT $2;
    "#;

    let mut client = pool.get().await?;
    let tx = client.transaction().await?;
    let rows = tx.query(QUERY, &[&now, &(limit as i64)]).await?;
    let candidates = rows
        .iter()
        .map(|row| -> Result<_> {
            Ok(EmbeddingGenerationCandidate {
                bookmark_id: row.try_get("bookmark_id").map_err(Error::from)?,
                user_id: row.try_get("user_id").map_err(Error::from)?,
                text_content: row.try_get("text_content").map_err(Error::from)?,
                attempts: row.try_get("embedding_attempts").map_err(Error::from)?,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let claimed_until = now + claim_window;
    for candidate in &candidates {
        tx.execute(
            "UPDATE bookmark
             SET embedding_next_attempt_at = $1
             WHERE bookmark_id = $2 AND user_id = $3",
            &[&claimed_until, &candidate.bookmark_id, &candidate.user_id],
        )
        .await?;
    }
    tx.commit().await?;

    if !candidates.is_empty() {
        info!(
            task_count = candidates.len(),
            claimed_until = %claimed_until,
            "Claimed bookmarks for embedding generation"
        );
    }
    Ok(candidates)
}

pub async fn refresh_text_ai_claim(
    pool: &PgPool,
    user_id: Uuid,
    bookmark_id: &str,
    next_attempt_at: DateTime<Utc>,
) -> Result<()> {
    pool.get()
        .await?
        .execute(
            "UPDATE bookmark
             SET text_ai_next_attempt_at = $1
             WHERE bookmark_id = $2 AND user_id = $3 AND text_ai_status = 'pending'",
            &[&next_attempt_at, &bookmark_id, &user_id],
        )
        .await?;
    Ok(())
}

pub async fn refresh_embedding_claim(
    pool: &PgPool,
    user_id: Uuid,
    bookmark_id: &str,
    next_attempt_at: DateTime<Utc>,
) -> Result<()> {
    pool.get()
        .await?
        .execute(
            "UPDATE bookmark
             SET embedding_next_attempt_at = $1
             WHERE bookmark_id = $2 AND user_id = $3 AND embedding_status = 'pending'",
            &[&next_attempt_at, &bookmark_id, &user_id],
        )
        .await?;
    Ok(())
}

pub async fn list_bookmark_ai_chunks(
    pool: &PgPool,
    user_id: Uuid,
    bookmark_id: &str,
) -> Result<Vec<BookmarkAiChunk>> {
    let client = pool.get().await?;
    let rows = client
        .query(
            "SELECT bookmark_id, user_id, chunk_index, chunk_hash, pipeline_version, summary, tags
             FROM bookmark_ai_chunk
             WHERE bookmark_id = $1 AND user_id = $2
             ORDER BY chunk_index ASC",
            &[&bookmark_id, &user_id],
        )
        .await?;

    rows.into_iter()
        .map(|row| {
            Ok(BookmarkAiChunk {
                bookmark_id: row.try_get("bookmark_id").map_err(Error::from)?,
                user_id: row.try_get("user_id").map_err(Error::from)?,
                chunk_index: row.try_get("chunk_index").map_err(Error::from)?,
                chunk_hash: row.try_get("chunk_hash").map_err(Error::from)?,
                pipeline_version: row.try_get("pipeline_version").map_err(Error::from)?,
                summary: row.try_get("summary").map_err(Error::from)?,
                tags: row.try_get("tags").map_err(Error::from)?,
            })
        })
        .collect()
}

pub async fn upsert_bookmark_ai_chunk(pool: &PgPool, chunk: &BookmarkAiChunk) -> Result<()> {
    pool.get()
        .await?
        .execute(
            r#"
            INSERT INTO bookmark_ai_chunk
                (bookmark_id, user_id, chunk_index, chunk_hash, pipeline_version, summary, tags, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, now(), now())
            ON CONFLICT (bookmark_id, user_id, chunk_index)
            DO UPDATE SET
                chunk_hash = EXCLUDED.chunk_hash,
                pipeline_version = EXCLUDED.pipeline_version,
                summary = EXCLUDED.summary,
                tags = EXCLUDED.tags,
                updated_at = now()
            "#,
            &[
                &chunk.bookmark_id,
                &chunk.user_id,
                &chunk.chunk_index,
                &chunk.chunk_hash,
                &chunk.pipeline_version,
                &chunk.summary,
                &chunk.tags,
            ],
        )
        .await?;
    Ok(())
}

pub async fn delete_bookmark_ai_chunks(
    pool: &PgPool,
    user_id: Uuid,
    bookmark_id: &str,
) -> Result<()> {
    let rows_affected = pool
        .get()
        .await?
        .execute(
            "DELETE FROM bookmark_ai_chunk WHERE bookmark_id = $1 AND user_id = $2",
            &[&bookmark_id, &user_id],
        )
        .await?;
    debug!(
        bookmark_id = %bookmark_id,
        user_id = %user_id,
        rows_affected,
        "Deleted stored unified AI chunk rows"
    );
    Ok(())
}

pub async fn mark_text_ai_failure(
    pool: &PgPool,
    user_id: Uuid,
    bookmark_id: &str,
    status: AiGenerationStatus,
    attempts: i16,
    next_attempt_at: DateTime<Utc>,
    fail_reason: &str,
) -> Result<()> {
    let fail_reason = truncate_fail_reason(fail_reason);
    pool.get()
        .await?
        .execute(
            "UPDATE bookmark
             SET text_ai_status = $1,
                 text_ai_attempts = $2,
                 text_ai_next_attempt_at = $3,
                 text_ai_fail_reason = $4,
                 updated_at = now()
             WHERE bookmark_id = $5 AND user_id = $6",
            &[
                &status,
                &attempts,
                &next_attempt_at,
                &fail_reason,
                &bookmark_id,
                &user_id,
            ],
        )
        .await?;
    Ok(())
}

pub async fn complete_text_ai_outputs(
    pool: &PgPool,
    user_id: Uuid,
    bookmark_id: &str,
    summary: Option<&str>,
    tags: Option<&[String]>,
    pipeline_version: i32,
) -> Result<()> {
    pool.get()
        .await?
        .execute(
            "UPDATE bookmark
             SET summary = $1,
                 tags = $2,
                 summary_status = 'done',
                 tag_status = 'done',
                 text_ai_status = 'done',
                 text_ai_attempts = 0,
                 text_ai_next_attempt_at = now(),
                 text_ai_fail_reason = NULL,
                 text_ai_pipeline_version = $3,
                 updated_at = now()
             WHERE bookmark_id = $4 AND user_id = $5",
            &[&summary, &tags, &pipeline_version, &bookmark_id, &user_id],
        )
        .await?;
    Ok(())
}

pub async fn mark_embedding_failure(
    pool: &PgPool,
    user_id: Uuid,
    bookmark_id: &str,
    status: AiGenerationStatus,
    attempts: i16,
    next_attempt_at: DateTime<Utc>,
    fail_reason: &str,
) -> Result<()> {
    let fail_reason = truncate_fail_reason(fail_reason);
    pool.get()
        .await?
        .execute(
            "UPDATE bookmark
             SET embedding_status = $1,
                 embedding_attempts = $2,
                 embedding_next_attempt_at = $3,
                 embedding_fail_reason = $4,
                 updated_at = now()
             WHERE bookmark_id = $5 AND user_id = $6",
            &[
                &status,
                &attempts,
                &next_attempt_at,
                &fail_reason,
                &bookmark_id,
                &user_id,
            ],
        )
        .await?;
    Ok(())
}

pub async fn mark_embedding_success(
    pool: &PgPool,
    user_id: Uuid,
    bookmark_id: &str,
    pipeline_version: i32,
) -> Result<()> {
    pool.get()
        .await?
        .execute(
            "UPDATE bookmark
             SET embedding_status = 'done',
                 embedding_attempts = 0,
                 embedding_next_attempt_at = now(),
                 embedding_fail_reason = NULL,
                 embedding_pipeline_version = $1,
                 updated_at = now()
             WHERE bookmark_id = $2 AND user_id = $3",
            &[&pipeline_version, &bookmark_id, &user_id],
        )
        .await?;
    Ok(())
}

pub async fn mark_embedding_done_without_chunks(
    pool: &PgPool,
    user_id: Uuid,
    bookmark_id: &str,
    pipeline_version: i32,
) -> Result<()> {
    mark_embedding_success(pool, user_id, bookmark_id, pipeline_version).await
}

pub async fn reset_embedding_generation_state(pool: &PgPool, pipeline_version: i32) -> Result<()> {
    pool.get()
        .await?
        .execute(
            "UPDATE bookmark
             SET embedding_status = CASE
                    WHEN LENGTH(text_content) >= 200 THEN 'pending'::task_status
                    ELSE 'done'::task_status
                 END,
                 embedding_attempts = 0,
                 embedding_next_attempt_at = now(),
                 embedding_fail_reason = NULL,
                 embedding_pipeline_version = $1,
                 updated_at = now()",
            &[&pipeline_version],
        )
        .await?;
    Ok(())
}
