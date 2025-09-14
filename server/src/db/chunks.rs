use chrono::{DateTime, Utc};
use deadpool_postgres::GenericClient;
use pgvector::Vector;
use postgres_from_row::FromRow;
use shared::{Bookmark, BookmarkChunk, RagChunkMatch};
use tracing::debug;
use uuid::Uuid;

use super::PgPool;
use crate::error::{Error, Result};

#[derive(Debug, FromRow)]
struct RowBookmarkChunk {
    chunk_id: Uuid,
    bookmark_id: String,
    user_id: Uuid,
    chunk_text: String,
    chunk_index: i32,
    created_at: DateTime<Utc>,
    updated_at: Option<DateTime<Utc>>,
}

impl From<RowBookmarkChunk> for BookmarkChunk {
    fn from(row: RowBookmarkChunk) -> Self {
        Self {
            chunk_id: row.chunk_id,
            bookmark_id: row.bookmark_id,
            user_id: row.user_id,
            chunk_text: row.chunk_text,
            chunk_index: row.chunk_index,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

pub async fn store_chunks_with_embeddings(
    pool: &PgPool,
    bookmark_id: &str,
    user_id: Uuid,
    chunks: Vec<String>,
    embeddings: Vec<Vec<f32>>,
) -> Result<Vec<BookmarkChunk>> {
    if chunks.len() != embeddings.len() {
        return Err(Error::bad_request([(
            "chunks",
            "Chunks and embeddings length mismatch",
        )]));
    }

    let client = pool.get().await?;

    // Delete existing chunks for this bookmark
    let _rows = client
        .execute(
            "DELETE FROM bookmark_chunk WHERE bookmark_id = $1 AND user_id = $2",
            &[&bookmark_id, &user_id],
        )
        .await?;

    debug!(
        bookmark_id,
        user_id = %user_id,
        chunk_count = chunks.len(),
        "Deleted existing chunks, inserting new ones"
    );

    let mut stored_chunks = Vec::new();

    for (index, (chunk_text, embedding)) in chunks.into_iter().zip(embeddings).enumerate() {
        let row = client
            .query_one(
                r#"
                INSERT INTO bookmark_chunk (bookmark_id, user_id, chunk_text, chunk_index, embedding)
                VALUES ($1, $2, $3, $4, $5)
                RETURNING chunk_id, bookmark_id, user_id, chunk_text, chunk_index, created_at, updated_at
                "#,
                &[&bookmark_id, &user_id, &chunk_text, &(index as i32), &Vector::from(embedding)],
            )
            .await?;

        let chunk_row = RowBookmarkChunk::try_from_row(&row).map_err(Error::from)?;
        stored_chunks.push(BookmarkChunk::from(chunk_row));
    }

    debug!(
        bookmark_id,
        user_id = %user_id,
        stored_count = stored_chunks.len(),
        "Successfully stored chunks with embeddings"
    );

    Ok(stored_chunks)
}

pub async fn search_similar_chunks(
    pool: &PgPool,
    user_id: Uuid,
    query_embedding: Vec<f32>,
    limit: usize,
    similarity_threshold: f64,
) -> Result<Vec<RagChunkMatch>> {
    let client = pool.get().await?;

    let rows = client
        .query(
            r#"
            SELECT 
                c.chunk_id, c.bookmark_id, c.user_id, c.chunk_text, 
                c.chunk_index, c.created_at, c.updated_at,
                b.url, b.domain, b.title, b.tags, b.summary, 
                b.created_at as bookmark_created_at, b.updated_at as bookmark_updated_at,
                1 - (c.embedding <=> $2) as similarity_score
            FROM bookmark_chunk c
            INNER JOIN bookmark b ON c.bookmark_id = b.bookmark_id AND c.user_id = b.user_id
            WHERE c.user_id = $1 
            AND 1 - (c.embedding <=> $2) >= $3
            ORDER BY c.embedding <=> $2
            LIMIT $4
            "#,
            &[
                &user_id,
                &Vector::from(query_embedding),
                &similarity_threshold,
                &(limit as i64),
            ],
        )
        .await?;

    let mut matches = Vec::new();
    for row in rows {
        let chunk = BookmarkChunk {
            chunk_id: row.get("chunk_id"),
            bookmark_id: row.get("bookmark_id"),
            user_id: row.get("user_id"),
            chunk_text: row.get("chunk_text"),
            chunk_index: row.get("chunk_index"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        };

        let bookmark = Bookmark {
            bookmark_id: row.get("bookmark_id"),
            user_id: row.get("user_id"),
            url: row.get("url"),
            domain: row.get("domain"),
            title: row.get("title"),
            tags: row.get("tags"),
            summary: row.get("summary"),
            created_at: row.get("bookmark_created_at"),
            updated_at: row.get("bookmark_updated_at"),
        };

        matches.push(RagChunkMatch {
            chunk,
            bookmark,
            similarity_score: row.get("similarity_score"),
            relevance_explanation: None, // Will be filled by relevance assessment
        });
    }

    debug!(
        user_id = %user_id,
        matches_found = matches.len(),
        similarity_threshold,
        "Found similar chunks"
    );

    Ok(matches)
}

pub async fn get_chunks_by_ids(
    pool: &PgPool,
    user_id: Uuid,
    chunk_ids: &[Uuid],
) -> Result<Vec<BookmarkChunk>> {
    if chunk_ids.is_empty() {
        return Ok(Vec::new());
    }

    let client = pool.get().await?;

    let rows = client
        .query(
            r#"
            SELECT chunk_id, bookmark_id, user_id, chunk_text, 
                   chunk_index, created_at, updated_at
            FROM bookmark_chunk 
            WHERE user_id = $1 AND chunk_id = ANY($2)
            ORDER BY chunk_index
            "#,
            &[&user_id, &chunk_ids],
        )
        .await?;

    let chunks: Result<Vec<_>> = rows
        .into_iter()
        .map(|row| {
            let chunk_row = RowBookmarkChunk::try_from_row(&row).map_err(Error::from)?;
            Ok(BookmarkChunk::from(chunk_row))
        })
        .collect();

    chunks
}

pub async fn has_chunks_for_bookmark(
    pool: &PgPool,
    bookmark_id: &str,
    user_id: Uuid,
) -> Result<bool> {
    let client = pool.get().await?;

    let count: i64 = client
        .query_one(
            "SELECT COUNT(*) FROM bookmark_chunk WHERE bookmark_id = $1 AND user_id = $2",
            &[&bookmark_id, &user_id],
        )
        .await?
        .get(0);

    Ok(count > 0)
}

pub async fn get_bookmarks_without_chunks(
    pool: &PgPool,
    limit: usize,
) -> Result<Vec<(String, Uuid, String)>> {
    let client = pool.get().await?;

    let rows = client
        .query(
            r#"
            SELECT b.bookmark_id, b.user_id, b.text_content
            FROM bookmark b
            LEFT JOIN bookmark_chunk c ON b.bookmark_id = c.bookmark_id AND b.user_id = c.user_id
            WHERE c.bookmark_id IS NULL
            AND LENGTH(b.text_content) > 100
            LIMIT $1
            "#,
            &[&(limit as i64)],
        )
        .await?;

    let results: Result<Vec<_>> = rows
        .into_iter()
        .map(|row| {
            Ok((
                row.get::<_, String>("bookmark_id"),
                row.get::<_, Uuid>("user_id"),
                row.get::<_, String>("text_content"),
            ))
        })
        .collect();

    results
}
