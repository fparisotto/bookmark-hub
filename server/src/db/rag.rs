use chrono::{DateTime, Utc};
use deadpool_postgres::GenericClient;
use postgres_from_row::FromRow;
use shared::{RagHistoryRequest, RagHistoryResponse, RagSession};
use tracing::debug;
use uuid::Uuid;

use super::PgPool;
use crate::error::{Error, Result};

#[derive(Debug, FromRow)]
struct RowRagSession {
    session_id: Uuid,
    user_id: Uuid,
    question: String,
    answer: Option<String>,
    relevant_chunks: Vec<Uuid>,
    created_at: DateTime<Utc>,
    updated_at: Option<DateTime<Utc>>,
}

impl From<RowRagSession> for RagSession {
    fn from(row: RowRagSession) -> Self {
        Self {
            session_id: row.session_id,
            user_id: row.user_id,
            question: row.question,
            answer: row.answer,
            relevant_chunks: row.relevant_chunks,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

pub async fn create_rag_session(
    pool: &PgPool,
    user_id: Uuid,
    question: &str,
) -> Result<RagSession> {
    let client = pool.get().await?;

    let row = client
        .query_one(
            r#"
            INSERT INTO rag_session (user_id, question)
            VALUES ($1, $2)
            RETURNING session_id, user_id, question, answer, relevant_chunks, created_at, updated_at
            "#,
            &[&user_id, &question],
        )
        .await?;

    let session_row = RowRagSession::try_from_row(&row).map_err(Error::from)?;
    debug!(
        session_id = %session_row.session_id,
        user_id = %user_id,
        "Created new RAG session"
    );

    Ok(RagSession::from(session_row))
}

pub async fn update_rag_session(
    pool: &PgPool,
    session_id: Uuid,
    user_id: Uuid,
    answer: &str,
    relevant_chunk_ids: &[Uuid],
) -> Result<RagSession> {
    let client = pool.get().await?;

    let row = client
        .query_one(
            r#"
            UPDATE rag_session 
            SET answer = $3, relevant_chunks = $4, updated_at = NOW()
            WHERE session_id = $1 AND user_id = $2
            RETURNING session_id, user_id, question, answer, relevant_chunks, created_at, updated_at
            "#,
            &[&session_id, &user_id, &answer, &relevant_chunk_ids],
        )
        .await?;

    let session_row = RowRagSession::try_from_row(&row).map_err(Error::from)?;
    debug!(
        session_id = %session_id,
        user_id = %user_id,
        chunks_count = relevant_chunk_ids.len(),
        "Updated RAG session with answer"
    );

    Ok(RagSession::from(session_row))
}

pub async fn get_rag_session(
    pool: &PgPool,
    session_id: Uuid,
    user_id: Uuid,
) -> Result<Option<RagSession>> {
    let client = pool.get().await?;

    let rows = client
        .query(
            r#"
            SELECT session_id, user_id, question, answer, relevant_chunks, created_at, updated_at
            FROM rag_session 
            WHERE session_id = $1 AND user_id = $2
            "#,
            &[&session_id, &user_id],
        )
        .await?;

    if let Some(row) = rows.first() {
        let session_row = RowRagSession::try_from_row(row).map_err(Error::from)?;
        Ok(Some(RagSession::from(session_row)))
    } else {
        Ok(None)
    }
}

pub async fn get_rag_history(
    pool: &PgPool,
    user_id: Uuid,
    request: &RagHistoryRequest,
) -> Result<RagHistoryResponse> {
    let client = pool.get().await?;

    let limit = request.limit.unwrap_or(20).min(100) as i64;
    let offset = request.offset.unwrap_or(0) as i64;

    // Get total count
    let count_row = client
        .query_one(
            "SELECT COUNT(*) FROM rag_session WHERE user_id = $1",
            &[&user_id],
        )
        .await?;
    let total_count: i64 = count_row.get(0);

    // Get sessions
    let rows = client
        .query(
            r#"
            SELECT session_id, user_id, question, answer, relevant_chunks, created_at, updated_at
            FROM rag_session 
            WHERE user_id = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
            &[&user_id, &limit, &offset],
        )
        .await?;

    let sessions: Result<Vec<_>> = rows
        .into_iter()
        .map(|row| {
            let session_row = RowRagSession::try_from_row(&row).map_err(Error::from)?;
            Ok(RagSession::from(session_row))
        })
        .collect();

    debug!(
        user_id = %user_id,
        sessions_count = sessions.as_ref().map(|s| s.len()).unwrap_or(0),
        total_count,
        "Retrieved RAG history"
    );

    Ok(RagHistoryResponse {
        sessions: sessions?,
        total_count: total_count as usize,
    })
}
