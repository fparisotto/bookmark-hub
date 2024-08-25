use chrono::{DateTime, Duration, Utc};
use deadpool_postgres::GenericClient;
use postgres_from_row::FromRow;
use postgres_types::{FromSql, ToSql};
use serde::{Deserialize, Serialize};
use tracing::instrument;
use url::Url;
use uuid::Uuid;

use crate::error::{self, Result};

use super::PgPool;

const TASK_MAX_RETRIES: i16 = 5;
const NEXT_DELIVERY_WINDOW: Duration = Duration::minutes(5);

#[derive(Debug, Clone, Serialize, Deserialize, FromSql, ToSql)]
#[postgres(name = "task_status", rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    Done,
    Fail,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Task {
    pub task_id: Uuid,
    pub user_id: Uuid,
    pub url: String,
    pub status: TaskStatus,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub next_delivery: DateTime<Utc>,
    pub retries: Option<i16>,
    pub fail_reason: Option<String>,
}

impl Task {
    pub fn should_retry(&self) -> bool {
        self.retries.unwrap_or(0) < TASK_MAX_RETRIES
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct BookmarkUserTask {
    pub task_id: Uuid,
    pub user_id: Uuid,
    pub bookmark_id: Uuid,
    pub status: TaskStatus,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[instrument(skip(pool))]
pub async fn create(pool: &PgPool, user_id: Uuid, url: Url, tags: Vec<String>) -> Result<Task> {
    const SQL: &str = r#"INSERT INTO "bookmark_task" (user_id, url, status, tags)
    VALUES ($1, $2, $3, $4) RETURNING "bookmark_task".*;"#;
    let client = pool.get().await?;
    let row = client
        .query_one(
            SQL,
            &[&user_id, &url.to_string(), &TaskStatus::Pending, &tags],
        )
        .await?;
    let task = Task::try_from_row(&row)?;
    Ok(task)
}

#[instrument(skip(pool))]
pub async fn peek(pool: &PgPool, now: DateTime<Utc>) -> Result<Vec<Task>> {
    const QUERY: &str = r#"SELECT * FROM bookmark_task WHERE next_delivery <= $1
    AND status = 'pending' FOR UPDATE SKIP LOCKED LIMIT 10;"#;

    let mut client = pool.get().await?;
    let tx = client.transaction().await?;

    let tasks = tx
        .query(QUERY, &[&now])
        .await?
        .iter()
        .map(|row| Task::try_from_row(row).map_err(error::Error::from))
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

#[instrument(skip(pool))]
pub async fn update(
    pool: &PgPool,
    task: Task,
    status: TaskStatus,
    retries: Option<i16>,
    fail_reason: Option<String>,
) -> Result<()> {
    const SQL: &str =
        "UPDATE bookmark_task SET status = $1, retries = $2, fail_reason = $3 WHERE task_id = $4";
    let client = pool.get().await?;
    let row_count = client
        .execute(SQL, &[&status, &retries, &fail_reason, &task.task_id])
        .await?;
    tracing::info!("Task updated, rows affected = {row_count}");
    Ok(())
}
