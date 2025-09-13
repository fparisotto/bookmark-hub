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

use super::PgPool;
use crate::error::{self, Error, Result};

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
    pub tags: Option<Vec<String>>,
    pub summary: Option<String>,
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
            summary: value.summary,
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
    let mut filters: Vec<String> = vec![];
    let mut params: Vec<&(dyn ToSql + Sync)> = vec![];

    params.push(&user_id);
    filters.push(format!("bt.user_id = ${}", params.len()));

    let url_pattern = request.url.as_ref().map(|e| format!("%{e}%"));
    if let Some(url_pattern) = &url_pattern {
        params.push(url_pattern);
        filters.push(format!("bt.url LIKE ${}", params.len()));
    }

    if let Some(tags) = &request.tags {
        if !tags.is_empty() {
            params.push(tags);
            filters.push(format!("bt.tags @> ${}", params.len()));
        }
    }

    let bookmark_task_status = request.status.clone().map(ColumnBookmarkTaskStatus::from);
    if let Some(status) = &bookmark_task_status {
        params.push(status);
        filters.push(format!("bt.status = ${}", params.len()));
    }

    if let Some(last_task_id) = &request.last_task_id {
        params.push(last_task_id);
        // This requires an ORDER BY clause for stable pagination
        filters.push(format!("bt.task_id > ${}", params.len()));
    }

    if let Some(to_created_at) = &request.to_created_at {
        params.push(to_created_at);
        filters.push(format!("bt.created_at <= ${}", params.len()));
    }

    if let Some(from_created_at) = &request.from_created_at {
        params.push(from_created_at);
        filters.push(format!("bt.created_at >= ${}", params.len()));
    }

    let filter_clause = if !filters.is_empty() {
        format!("WHERE {}", filters.join(" AND "))
    } else {
        String::new()
    };

    let page_size = request.page_size.unwrap_or(25) as usize;
    // Order by task_id for stable pagination using `last_task_id`
    // Fetch one extra row to determine if there are more results
    let sql = format!(
        "SELECT * FROM bookmark_task bt {filter_clause} ORDER BY bt.task_id ASC LIMIT {}",
        page_size + 1
    );

    let client = pool.get().await?;
    let rows = client.query(&sql, &params).await?;

    let mut tasks = rows
        .iter()
        .map(|row| {
            RowBookmarkTask::try_from_row(row)
                .map(BookmarkTask::from)
                .map_err(Error::from)
        })
        .collect::<Result<Vec<_>>>()?;

    // Check if we have more results
    let has_more = tasks.len() > page_size;
    if has_more {
        // Remove the extra row
        tasks.truncate(page_size);
    }

    Ok(BookmarkTaskSearchResponse {
        tasks,
        has_more,
        total_count: None, // Could add a COUNT query if needed
    })
}
