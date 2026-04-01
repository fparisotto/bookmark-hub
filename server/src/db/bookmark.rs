use std::collections::HashSet;

use anyhow::anyhow;
use chrono::{DateTime, Utc};
use deadpool_postgres::GenericClient;
use postgres_from_row::FromRow;
use postgres_types::{FromSql, ToSql};
use serde::{Deserialize, Serialize};
use shared::{Bookmark, TagOperation};
use tracing::{debug, info};
use uuid::Uuid;

use super::{PgPool, ResultExt};
use crate::bookmark_identity::canonicalize_url_str;
use crate::error::{Error, Result};

const MAX_FAILURE_REASON_LEN: usize = 2048;

fn normalize_tags(tags: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    tags.iter()
        .map(|t| t.trim().to_lowercase())
        .filter(|t| !t.is_empty() && seen.insert(t.clone()))
        .collect()
}

fn status_for_initial_tags(tags: Option<&[String]>) -> AiGenerationStatus {
    match tags {
        Some(tags) if !tags.is_empty() => AiGenerationStatus::Done,
        _ => AiGenerationStatus::Pending,
    }
}

fn status_for_initial_summary(summary: Option<&str>) -> AiGenerationStatus {
    match summary {
        Some(summary) if !summary.trim().is_empty() => AiGenerationStatus::Done,
        _ => AiGenerationStatus::Pending,
    }
}

fn truncate_fail_reason(fail_reason: &str) -> String {
    if fail_reason.chars().count() <= MAX_FAILURE_REASON_LEN {
        return fail_reason.to_string();
    }

    fail_reason.chars().take(MAX_FAILURE_REASON_LEN).collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, FromSql, ToSql)]
#[postgres(name = "task_status", rename_all = "snake_case")]
pub enum AiGenerationStatus {
    Pending,
    Done,
    Fail,
}

#[derive(Debug, Clone)]
pub struct BookmarkGenerationCandidate {
    pub bookmark: Bookmark,
    pub attempts: i16,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
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
            url: value.url,
            domain: value.domain,
            title: value.title,
            user_id: value.user_id,
            tags: value.tags,
            summary: value.summary,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

pub async fn get_tag_count_by_user(pool: &PgPool, user_id: Uuid) -> Result<Vec<(String, i64)>> {
    const SQL: &str = r#"
    WITH tags AS (
        SELECT unnest(tags) AS tag
        FROM bookmark
        WHERE user_id = $1
    )
    SELECT tag, count(1) AS counter FROM tags GROUP BY tag;
    "#;
    debug!(user_id = %user_id, "Fetching tag counts");
    let client = pool.get().await?;
    let rows = client.query(SQL, &[&user_id]).await?;
    let result = rows
        .iter()
        .map(|row| {
            let tag = row.try_get::<usize, String>(0);
            let counter = row.try_get::<usize, i64>(1);
            tag.and_then(|t| counter.map(|c| (t, c)))
                .map_err(Error::from)
        })
        .collect::<Result<Vec<_>>>()?;
    debug!(user_id = %user_id, tag_count = %result.len(), "Found unique tags");
    Ok(result)
}

pub async fn get_by_user(pool: &PgPool, user_id: Uuid) -> Result<Vec<Bookmark>> {
    const SQL: &str = "SELECT * FROM bookmark b WHERE b.user_id = $1 ORDER BY b.created_at ASC;";
    debug!(user_id = %user_id, "Fetching all bookmarks");
    let client = pool.get().await?;
    let results = client
        .query(SQL, &[&user_id])
        .await?
        .iter()
        .map(|row| {
            RowBookmark::try_from_row(row)
                .map(Bookmark::from)
                .map_err(Error::from)
        })
        .collect::<Result<Vec<_>>>()?;
    info!(user_id = %user_id, bookmark_count = %results.len(), "Retrieved bookmarks");
    Ok(results)
}

pub async fn get_by_tag(pool: &PgPool, user_id: Uuid, tag: &str) -> Result<Vec<Bookmark>> {
    const SQL: &str =
        "SELECT * FROM bookmark b WHERE b.user_id = $1 AND b.tags @> $2 ORDER BY b.created_at ASC;";
    debug!(user_id = %user_id, tag = %tag, "Fetching bookmarks with tag");
    let client = pool.get().await?;
    let results = client
        .query(SQL, &[&user_id, &[&tag]])
        .await?
        .iter()
        .map(|row| {
            RowBookmark::try_from_row(row)
                .map(Bookmark::from)
                .map_err(Error::from)
        })
        .collect::<Result<Vec<_>>>()?;
    info!(user_id = %user_id, tag = %tag, bookmark_count = %results.len(), "Found bookmarks with tag");
    Ok(results)
}

pub async fn get_by_canonical_url_and_user_id(
    pool: &PgPool,
    url: &str,
    user_id: Uuid,
) -> Result<Option<Bookmark>> {
    const SQL: &str = "SELECT * FROM bookmark WHERE canonical_url = $1 AND user_id = $2;";
    let canonical_url = canonicalize_url_str(url)?;
    debug!(url = %url, canonical_url = %canonical_url, user_id = %user_id, "Checking for existing bookmark");
    let client = pool.get().await?;
    let result = client
        .query_opt(SQL, &[&canonical_url, &user_id])
        .await?
        .map(|row| {
            RowBookmark::try_from_row(&row)
                .map(Bookmark::from)
                .map_err(Error::from)
        })
        .transpose()?;
    match &result {
        Some(bookmark) => {
            debug!(
                bookmark_id = %bookmark.bookmark_id,
                canonical_url = %canonical_url,
                "Found existing bookmark"
            )
        }
        None => debug!(canonical_url = %canonical_url, "No existing bookmark found"),
    }
    Ok(result)
}

pub async fn get_with_user_data(
    pool: &PgPool,
    user_id: Uuid,
    bookmark_id: &str,
) -> Result<Option<Bookmark>> {
    const SQL: &str = "SELECT * FROM bookmark b WHERE b.user_id = $1 AND b.bookmark_id = $2;";
    debug!(bookmark_id = %bookmark_id, user_id = %user_id, "Fetching bookmark");
    let client = pool.get().await?;
    let result = client
        .query_opt(SQL, &[&user_id, &bookmark_id])
        .await?
        .map(|row| {
            RowBookmark::try_from_row(&row)
                .map(Bookmark::from)
                .map_err(Error::from)
        })
        .transpose()?;
    match &result {
        Some(_) => debug!(bookmark_id = %bookmark_id, "Found bookmark"),
        None => debug!(bookmark_id = %bookmark_id, "Bookmark not found for user"),
    }
    Ok(result)
}

pub async fn update_tags(
    pool: &PgPool,
    user_id: Uuid,
    bookmark_id: &str,
    operation: &TagOperation,
) -> Result<Bookmark> {
    let (update_tag_sql, tags) = match operation {
        TagOperation::Set(tags) => ("tags=$1", normalize_tags(tags)),
        TagOperation::Append(tags) => (
            "tags=(SELECT ARRAY(SELECT DISTINCT unnest(array_cat(COALESCE(tags, ARRAY[]::text[]), $1))))",
            normalize_tags(tags),
        ),
    };
    let sql = format!(
        "UPDATE bookmark SET {update_tag_sql}, tag_status='done', tag_attempts=0, tag_next_attempt_at=now(), tag_fail_reason=NULL, updated_at=now() WHERE bookmark_id=$2 AND user_id=$3 RETURNING *;"
    );
    let client = pool.get().await?;
    let row = client
        .query_one(&sql, &[&tags, &bookmark_id, &user_id])
        .await?;
    let result = RowBookmark::try_from_row(&row)
        .map(Bookmark::from)
        .map_err(Error::from)?;
    info!(
        bookmark_id = %bookmark_id,
        user_id = %user_id,
        operation = ?operation,
        new_tag_count = %result.tags.as_ref().map(|t| t.len()).unwrap_or(0),
        "Updated tags for bookmark"
    );
    Ok(result)
}

pub async fn save(pool: &PgPool, bookmark: &Bookmark, text_content: &str) -> Result<Bookmark> {
    let canonical_url = canonicalize_url_str(&bookmark.url)?;
    const SQL: &str = r#"
    INSERT INTO bookmark
    (bookmark_id, user_id, url, canonical_url, domain, title, text_content, tags, summary, summary_status, tag_status, created_at, updated_at)
    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, now(), now()) RETURNING *;"#;
    let normalized_tags: Option<Vec<String>> = bookmark.tags.as_ref().map(|t| normalize_tags(t));
    let summary_status = status_for_initial_summary(bookmark.summary.as_deref());
    let tag_status = status_for_initial_tags(normalized_tags.as_deref());
    let client = pool.get().await?;
    let row = client
        .query_one(
            SQL,
            &[
                &bookmark.bookmark_id,
                &bookmark.user_id,
                &bookmark.url,
                &canonical_url,
                &bookmark.domain,
                &bookmark.title,
                &text_content,
                &normalized_tags,
                &bookmark.summary,
                &summary_status,
                &tag_status,
            ],
        )
        .await
        .on_constraint("bookmark_canonical_url_user_unique", |_| {
            Error::constraint_violation("duplicate_bookmark", "bookmark already exists for user")
        })?;
    let result = RowBookmark::try_from_row(&row)
        .map(Bookmark::from)
        .map_err(Error::from)?;
    info!(
        bookmark_id = %result.bookmark_id,
        user_id = %result.user_id,
        url = %result.url,
        title = %result.title,
        "Bookmark saved"
    );
    Ok(result)
}

async fn bookmark_has_canonical_url_column(client: &impl GenericClient) -> Result<bool> {
    let exists = client
        .query_one(
            r#"
            SELECT EXISTS (
                SELECT 1
                FROM information_schema.columns
                WHERE table_schema = current_schema()
                  AND table_name = 'bookmark'
                  AND column_name = 'canonical_url'
            );
            "#,
            &[],
        )
        .await?
        .get(0);
    Ok(exists)
}

pub async fn ensure_canonical_url_support(pool: &PgPool) -> Result<()> {
    let mut client = pool.get().await?;
    if !bookmark_has_canonical_url_column(&client).await? {
        return Ok(());
    }

    let tx = client.transaction().await?;
    let rows = tx
        .query(
            r#"
            SELECT bookmark_id, user_id, url
            FROM bookmark
            WHERE canonical_url IS NULL
            ORDER BY created_at ASC;
            "#,
            &[],
        )
        .await?;

    for row in rows {
        let bookmark_id: String = row.get("bookmark_id");
        let user_id: Uuid = row.get("user_id");
        let url: String = row.get("url");
        let canonical_url = canonicalize_url_str(&url)?;
        tx.execute(
            "UPDATE bookmark SET canonical_url = $1 WHERE bookmark_id = $2 AND user_id = $3",
            &[&canonical_url, &bookmark_id, &user_id],
        )
        .await?;
    }

    if let Some(row) = tx
        .query_opt(
            r#"
            SELECT user_id, canonical_url, COUNT(*) AS row_count
            FROM bookmark
            WHERE canonical_url IS NOT NULL
            GROUP BY user_id, canonical_url
            HAVING COUNT(*) > 1
            LIMIT 1;
            "#,
            &[],
        )
        .await?
    {
        let user_id: Uuid = row.get("user_id");
        let canonical_url: String = row.get("canonical_url");
        let row_count: i64 = row.get("row_count");
        return Err(anyhow!(
            "bookmark canonical_url collision for user_id={user_id}, canonical_url={canonical_url}, row_count={row_count}"
        )
        .into());
    }

    tx.batch_execute(
        r#"
        ALTER TABLE bookmark ALTER COLUMN canonical_url SET NOT NULL;
        DO $$
        BEGIN
            IF NOT EXISTS (
                SELECT 1
                FROM pg_constraint
                WHERE conname = 'bookmark_canonical_url_user_unique'
            ) THEN
                ALTER TABLE bookmark
                ADD CONSTRAINT bookmark_canonical_url_user_unique UNIQUE (user_id, canonical_url);
            END IF;
        END $$;
        "#,
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn update_summary(
    pool: &PgPool,
    user_id: Uuid,
    bookmark_id: &str,
    summary: &str,
) -> Result<Bookmark> {
    const SQL: &str = "UPDATE bookmark SET summary = $1, summary_status='done', summary_attempts=0, summary_next_attempt_at=now(), summary_fail_reason=NULL, updated_at=now() WHERE bookmark_id=$2 AND user_id=$3 RETURNING *;";
    let client = pool.get().await?;
    let row = client
        .query_one(SQL, &[&summary, &bookmark_id, &user_id])
        .await?;
    let result = RowBookmark::try_from_row(&row)
        .map(Bookmark::from)
        .map_err(Error::from)?;
    info!(
        bookmark_id = %bookmark_id,
        user_id = %user_id,
        summary_length = %summary.len(),
        "Updated summary for bookmark"
    );
    Ok(result)
}

pub async fn get_text_content(
    pool: &PgPool,
    user_id: Uuid,
    bookmark_id: &str,
) -> Result<Option<String>> {
    const SQL: &str = "SELECT text_content FROM bookmark WHERE bookmark_id = $1 AND user_id = $2";
    debug!(bookmark_id = %bookmark_id, user_id = %user_id, "Fetching text content");
    let client = pool.get().await?;
    let result: Option<String> = client
        .query_opt(SQL, &[&bookmark_id, &user_id])
        .await?
        .map(|row| row.try_get(0))
        .transpose()?;
    match &result {
        Some(content) => {
            debug!(bookmark_id = %bookmark_id, content_length = %content.len(), "Found text content")
        }
        None => debug!(bookmark_id = %bookmark_id, "No text content found"),
    }
    Ok(result)
}

pub async fn get_bookmarks_pending_tag_generation(
    pool: &PgPool,
    limit: usize,
    now: DateTime<Utc>,
) -> Result<Vec<BookmarkGenerationCandidate>> {
    const SQL: &str = r#"
        SELECT *
        FROM bookmark
        WHERE tag_status = $1
          AND tag_next_attempt_at <= $2
          AND (tags IS NULL OR coalesce(array_length(tags, 1), 0) = 0)
        ORDER BY tag_next_attempt_at ASC, created_at ASC
        LIMIT $3;
    "#;
    debug!(limit = %limit, current_time = %now, "Fetching bookmarks pending tag generation");
    let client = pool.get().await?;
    let status = AiGenerationStatus::Pending;
    let results = client
        .query(SQL, &[&status, &now, &(limit as i64)])
        .await?
        .iter()
        .map(|row| {
            let bookmark = RowBookmark::try_from_row(row)
                .map(Bookmark::from)
                .map_err(Error::from)?;
            let attempts: i16 = row.try_get("tag_attempts").map_err(Error::from)?;
            Ok(BookmarkGenerationCandidate { bookmark, attempts })
        })
        .collect::<Result<Vec<_>>>()?;
    info!(
        found_count = %results.len(),
        limit = %limit,
        "Found bookmarks pending tag generation"
    );
    Ok(results)
}

pub async fn delete(pool: &PgPool, user_id: Uuid, bookmark_id: &str) -> Result<bool> {
    const SQL: &str = "DELETE FROM bookmark WHERE bookmark_id = $1 AND user_id = $2";
    debug!(bookmark_id = %bookmark_id, user_id = %user_id, "Deleting bookmark");
    let client = pool.get().await?;
    let rows_affected = client.execute(SQL, &[&bookmark_id, &user_id]).await?;
    if rows_affected > 0 {
        info!(bookmark_id = %bookmark_id, user_id = %user_id, "Bookmark deleted");
        Ok(true)
    } else {
        debug!(bookmark_id = %bookmark_id, user_id = %user_id, "Bookmark not found for deletion");
        Ok(false)
    }
}

pub async fn get_bookmarks_pending_summary_generation(
    pool: &PgPool,
    limit: usize,
) -> Result<Vec<BookmarkGenerationCandidate>> {
    let now = Utc::now();
    get_bookmarks_pending_summary_generation_at(pool, limit, now).await
}

pub async fn get_bookmarks_pending_summary_generation_at(
    pool: &PgPool,
    limit: usize,
    now: DateTime<Utc>,
) -> Result<Vec<BookmarkGenerationCandidate>> {
    const SQL: &str = r#"
        SELECT *
        FROM bookmark
        WHERE summary_status = $1
          AND summary_next_attempt_at <= $2
          AND summary IS NULL
        ORDER BY summary_next_attempt_at ASC, created_at ASC
        LIMIT $3;
    "#;
    debug!(limit = %limit, current_time = %now, "Fetching bookmarks pending summary generation");
    let client = pool.get().await?;
    let status = AiGenerationStatus::Pending;
    let results = client
        .query(SQL, &[&status, &now, &(limit as i64)])
        .await?
        .iter()
        .map(|row| {
            let bookmark = RowBookmark::try_from_row(row)
                .map(Bookmark::from)
                .map_err(Error::from)?;
            let attempts: i16 = row.try_get("summary_attempts").map_err(Error::from)?;
            Ok(BookmarkGenerationCandidate { bookmark, attempts })
        })
        .collect::<Result<Vec<_>>>()?;
    info!(
        found_count = %results.len(),
        limit = %limit,
        "Found bookmarks pending summary generation"
    );
    Ok(results)
}

pub async fn mark_summary_generation_failure(
    pool: &PgPool,
    user_id: Uuid,
    bookmark_id: &str,
    status: AiGenerationStatus,
    attempts: i16,
    next_attempt_at: DateTime<Utc>,
    fail_reason: &str,
) -> Result<()> {
    const SQL: &str = r#"
        UPDATE bookmark
        SET summary_status = $1,
            summary_attempts = $2,
            summary_next_attempt_at = $3,
            summary_fail_reason = $4,
            updated_at = now()
        WHERE bookmark_id = $5 AND user_id = $6;
    "#;
    let client = pool.get().await?;
    let fail_reason = truncate_fail_reason(fail_reason);
    let rows_affected = client
        .execute(
            SQL,
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
    info!(
        bookmark_id = %bookmark_id,
        user_id = %user_id,
        attempts = %attempts,
        status = ?status,
        rows_affected = %rows_affected,
        "Recorded summary generation failure"
    );
    Ok(())
}

pub async fn mark_tag_generation_failure(
    pool: &PgPool,
    user_id: Uuid,
    bookmark_id: &str,
    status: AiGenerationStatus,
    attempts: i16,
    next_attempt_at: DateTime<Utc>,
    fail_reason: &str,
) -> Result<()> {
    const SQL: &str = r#"
        UPDATE bookmark
        SET tag_status = $1,
            tag_attempts = $2,
            tag_next_attempt_at = $3,
            tag_fail_reason = $4,
            updated_at = now()
        WHERE bookmark_id = $5 AND user_id = $6;
    "#;
    let client = pool.get().await?;
    let fail_reason = truncate_fail_reason(fail_reason);
    let rows_affected = client
        .execute(
            SQL,
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
    info!(
        bookmark_id = %bookmark_id,
        user_id = %user_id,
        attempts = %attempts,
        status = ?status,
        rows_affected = %rows_affected,
        "Recorded tag generation failure"
    );
    Ok(())
}
