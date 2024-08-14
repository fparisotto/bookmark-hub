use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};
use tracing::instrument;
use url::Url;
use uuid::Uuid;

use crate::error::Result;

#[derive(Deserialize, Serialize, Debug, Copy, Clone, sqlx::Type)]
#[sqlx(type_name = "task_status")]
#[sqlx(rename_all = "lowercase")]
pub enum TaskStatus {
    Pending,
    Done,
    Fail,
}

#[derive(Debug, sqlx::FromRow, Serialize, Deserialize)]
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

#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct BookmarkUserTask {
    pub task_id: Uuid,
    pub user_id: Uuid,
    pub bookmark_id: Uuid,
    pub status: TaskStatus,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[instrument(skip(db))]
pub async fn create(
    db: &Pool<Postgres>,
    user_id: &Uuid,
    url: &Url,
    tags: &Vec<String>,
) -> Result<Task> {
    const SQL: &str = r#"
    insert into "bookmark_task" (user_id, url, status, tags)
    values ($1, $2, $3, $4) returning "bookmark_task".*;"#;
    let task: Task = sqlx::query_as(SQL)
        .bind(user_id)
        .bind(url.to_string())
        .bind(TaskStatus::Pending)
        .bind(tags)
        .fetch_one(db)
        .await?;
    Ok(task)
}

#[instrument(skip(pool))]
async fn peek(pool: &Pool<Postgres>, now: DateTime<Utc>) -> Result<Vec<Task>> {
    // https://softwaremill.com/mqperf/#postgresql
    let sql = r#"
    SELECT * FROM bookmark_task WHERE next_delivery <= $1 AND status = 'pending'
    FOR UPDATE SKIP LOCKED LIMIT 10
    "#;
    let mut tx = pool.begin().await?;
    let result: Vec<Task> = sqlx::query_as(sql).bind(now).fetch_all(&mut *tx).await?;
    let ids: Vec<Uuid> = result.iter().map(|t| t.task_id).collect();
    let sql = "UPDATE bookmark_task SET next_delivery = $1 WHERE task_id = ANY ($2)";
    let next_delivery = now + Duration::minutes(5);
    let update_result = sqlx::query(sql)
        .bind(next_delivery)
        .bind(&ids)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    tracing::debug!(
        tasks_ids = format!("{:?}", ids),
        nex_delivery = format!("{:?}", &next_delivery),
        rows_affected = update_result.rows_affected(),
        "Peek tasks, schedule for next delivery",
    );
    Ok(result)
}

#[instrument(skip(pool))]
async fn update(
    pool: &Pool<Postgres>,
    task: &Task,
    status: TaskStatus,
    retries: Option<i16>,
    fail_reason: Option<String>,
) -> Result<()> {
    let sql =
        "UPDATE bookmark_task SET status = $1, retries = $2, fail_reason = $3 WHERE task_id = $4";
    let update_result = sqlx::query(sql)
        .bind(status)
        .bind(retries)
        .bind(fail_reason)
        .bind(task.task_id)
        .execute(pool)
        .await?;
    tracing::info!("Task updated, info = {:?}", update_result);
    Ok(())
}
