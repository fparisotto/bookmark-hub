use chrono::{DateTime, Utc};
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
}

#[derive(Debug, sqlx::FromRow, Serialize)]
pub struct BookmarkTask {
    pub task_id: Uuid,
    pub user_id: Uuid,
    pub url: String,
    pub status: TaskStatus,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
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

pub struct BookmarkTaskTable;

impl BookmarkTaskTable {
    #[instrument(skip(db))]
    pub async fn create(
        db: &Pool<Postgres>,
        user_id: &Uuid,
        url: &Url,
        tags: &Vec<String>,
    ) -> Result<BookmarkTask> {
        let sql = r#"
            insert into "bookmark_task" (user_id, url, status, tags)
            values ($1, $2, $3, $4) returning "bookmark_task".*;
        "#;
        let task: BookmarkTask = sqlx::query_as(sql)
            .bind(user_id)
            .bind(url.to_string())
            .bind(TaskStatus::Pending)
            .bind(tags)
            .fetch_one(db)
            .await?;
        Ok(task)
    }
}
