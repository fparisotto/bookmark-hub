use chrono::{DateTime, Duration, Utc};
use deadpool_postgres::GenericClient;
use postgres_from_row::FromRow;
use postgres_types::{FromSql, ToSql};
use serde::{Deserialize, Serialize};
use shared::{
    BookmarkTask, BookmarkTaskSearchRequest, BookmarkTaskSearchResponse, BookmarkTaskStatus,
};
use url::Url;
use uuid::Uuid;

use crate::error::{self, Error, Result};

use super::PgPool;

const NEXT_DELIVERY_WINDOW: Duration = Duration::minutes(5);

#[derive(Debug, Clone, Serialize, Deserialize, FromSql, ToSql)]
#[postgres(name = "task_status", rename_all = "snake_case")]
enum ColumnBookmarkTaskStatus {
    Pending,
    Done,
    Fail,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
struct RowBookmarkTask {
    pub task_id: Uuid,
    pub user_id: Uuid,
    pub url: String,
    pub status: ColumnBookmarkTaskStatus,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub next_delivery: DateTime<Utc>,
    pub retries: Option<i16>,
    pub fail_reason: Option<String>,
}

impl From<ColumnBookmarkTaskStatus> for BookmarkTaskStatus {
    fn from(value: ColumnBookmarkTaskStatus) -> Self {
        match value {
            ColumnBookmarkTaskStatus::Pending => BookmarkTaskStatus::Pending,
            ColumnBookmarkTaskStatus::Done => BookmarkTaskStatus::Done,
            ColumnBookmarkTaskStatus::Fail => BookmarkTaskStatus::Fail,
        }
    }
}

impl From<BookmarkTaskStatus> for ColumnBookmarkTaskStatus {
    fn from(value: BookmarkTaskStatus) -> Self {
        match value {
            BookmarkTaskStatus::Pending => ColumnBookmarkTaskStatus::Pending,
            BookmarkTaskStatus::Done => ColumnBookmarkTaskStatus::Done,
            BookmarkTaskStatus::Fail => ColumnBookmarkTaskStatus::Fail,
        }
    }
}

impl From<RowBookmarkTask> for BookmarkTask {
    fn from(value: RowBookmarkTask) -> Self {
        Self {
            task_id: value.task_id,
            user_id: value.user_id,
            url: value.url,
            status: value.status.into(),
            tags: value.tags,
            created_at: value.created_at,
            updated_at: value.updated_at,
            next_delivery: value.next_delivery,
            retries: value.retries,
            fail_reason: value.fail_reason,
        }
    }
}

pub async fn create(
    pool: &PgPool,
    user_id: Uuid,
    url: Url,
    tags: Vec<String>,
) -> Result<BookmarkTask> {
    const SQL: &str = r#"INSERT INTO "bookmark_task" (user_id, url, status, tags)
    VALUES ($1, $2, $3, $4) RETURNING "bookmark_task".*;"#;
    let client = pool.get().await?;
    let row = client
        .query_one(
            SQL,
            &[
                &user_id,
                &url.to_string(),
                &ColumnBookmarkTaskStatus::Pending,
                &tags,
            ],
        )
        .await?;
    let task = RowBookmarkTask::try_from_row(&row)
        .map(BookmarkTask::from)
        .map_err(anyhow::Error::from)?;
    tracing::debug!(?task, "Task created");
    Ok(task)
}

pub async fn peek(pool: &PgPool, now: DateTime<Utc>) -> Result<Vec<BookmarkTask>> {
    const QUERY: &str = r#"SELECT * FROM bookmark_task WHERE next_delivery <= $1
    AND status = 'pending' FOR UPDATE SKIP LOCKED LIMIT 10;"#;

    let mut client = pool.get().await?;
    let tx = client.transaction().await?;

    let tasks = tx
        .query(QUERY, &[&now])
        .await?
        .iter()
        .map(|row| {
            RowBookmarkTask::try_from_row(row)
                .map(BookmarkTask::from)
                .map_err(error::Error::from)
        })
        .collect::<Result<Vec<_>>>()?;

    const UPDATE: &str = "UPDATE bookmark_task SET next_delivery = $1 WHERE task_id = ANY ($2);";
    let next_delivery = now + NEXT_DELIVERY_WINDOW;
    let ids: Vec<Uuid> = tasks.iter().map(|t| t.task_id).collect();
    let rows_affected = tx.execute(UPDATE, &[&next_delivery, &ids]).await?;
    tx.commit().await?;

    tracing::debug!(
        ?ids,
        ?next_delivery,
        %rows_affected,
        "Peek tasks, schedule for next delivery",
    );
    Ok(tasks)
}

pub async fn update(
    pool: &PgPool,
    task: BookmarkTask,
    status: BookmarkTaskStatus,
    retries: Option<i16>,
    fail_reason: Option<String>,
) -> Result<()> {
    const SQL: &str =
        "UPDATE bookmark_task SET status = $1, retries = $2, fail_reason = $3 WHERE task_id = $4";
    let client = pool.get().await?;
    let status: ColumnBookmarkTaskStatus = status.into();
    let row_count = client
        .execute(SQL, &[&status, &retries, &fail_reason, &task.task_id])
        .await?;
    tracing::info!("Task updated, rows affected = {row_count}");
    Ok(())
}

pub async fn search(
    pool: &PgPool,
    user_id: Uuid,
    request: &BookmarkTaskSearchRequest,
) -> Result<BookmarkTaskSearchResponse> {
    const SQL: &str = "SELECT * FROM bookmark_task bt WHERE bt.user_id = $1 AND ( ";
    let client = pool.get().await?;
    let mut query = SQL.to_owned();

    if request.url.is_some() {
        query.push_str(" bt.url LIKE $2 ");
    } else {
        query.push_str(" CAST($2 AS TEXT) IS NULL ");
    }

    if let Some(tags) = &request.tags {
        if !tags.is_empty() {
            query.push_str(" AND bt.tags @> $3 ");
        } else {
            query.push_str(" AND CAST($3 AS TEXT[]) IS NULL ");
        }
    } else {
        query.push_str(" AND CAST($3 AS TEXT[]) IS NULL ");
    }

    if request.status.is_some() {
        query.push_str(" AND bt.status = $4 ");
    } else {
        query.push_str(" AND CAST($4 AS task_status) IS NULL ");
    }

    if request.last_task_id.is_some() {
        query.push_str(" AND bt.task_id > $5 ");
    } else {
        query.push_str(" AND CAST($5 AS UUID) IS NULL ");
    }

    match (request.to_created_at, request.from_created_at) {
        (None, None) => {
            query.push_str(
                " AND CAST($6 AS TIMESTAMPTZ) IS NULL AND CAST($7 AS TIMESTAMPTZ) IS NULL ",
            );
        }
        (None, Some(_)) => {
            query.push_str(" AND CAST($6 AS TIMESTAMPTZ) IS NULL AND bt.created_at < $7 ");
        }
        (Some(_), None) => {
            query.push_str(" AND bt.created_at > $6 AND CAST($7 AS TIMESTAMPTZ) IS NULL ");
        }
        (Some(_), Some(_)) => {
            query.push_str(" AND bt.created_at BETWEEN $6 AND $7 ");
        }
    }

    let bookmark_task_status = request
        .status
        .to_owned()
        .map(ColumnBookmarkTaskStatus::from);

    let url_pattern = request.url.to_owned().map(|e| format!("%{e}%"));

    query.push_str(&format!(" ) LIMIT {}", request.page_size.unwrap_or(25)));

    let rows = client
        .query(
            &query,
            &[
                &user_id,
                &url_pattern,
                &request.tags,
                &bookmark_task_status,
                &request.last_task_id,
                &request.to_created_at,
                &request.from_created_at,
            ],
        )
        .await?;
    let tasks = rows
        .iter()
        .map(|row| {
            RowBookmarkTask::try_from_row(row)
                .map(BookmarkTask::from)
                .map_err(Error::from)
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(BookmarkTaskSearchResponse { tasks })
}
