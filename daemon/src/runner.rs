use anyhow::{Context, Result};
use aws_sdk_s3::types::ByteStream;
use aws_sdk_s3::Client as S3Client;
use chrono::{DateTime, Duration, Utc};
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};
use tokio::time::{sleep, Duration as TokioDuration};
use tracing::instrument;
use url::Url;
use uuid::Uuid;

use crate::processor;
use crate::processor::Bookmark;
use crate::Config;

const MAX_RETRIES: i16 = 5;

#[derive(Deserialize, Serialize, Debug, Copy, Clone, sqlx::Type)]
#[sqlx(type_name = "task_status")]
#[sqlx(rename_all = "lowercase")]
enum TaskStatus {
    Pending,
    Done,
    Fail,
}

#[derive(Debug, sqlx::FromRow, Serialize, Deserialize)]
struct Task {
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
    fn clean_url(&self) -> Result<String> {
        let url = Url::parse(&self.url)?;
        let cleaned_url = processor::clean_url(&url)?;
        Ok(cleaned_url.to_string())
    }
    fn should_retry(&self) -> bool {
        self.retries.unwrap_or(0) < MAX_RETRIES
    }
}

#[derive(Debug, sqlx::FromRow, Serialize, Deserialize)]
struct DatabaseBookmark {
    pub bookmark_id: String,
    pub url: String,
    pub domain: String,
    pub title: String,
    pub text_content: String,
    pub html_content: String,
    pub images: Vec<String>,
    pub created_at: DateTime<Utc>,
}

impl From<&Bookmark> for DatabaseBookmark {
    fn from(bookmark: &Bookmark) -> Self {
        Self {
            bookmark_id: bookmark.id.clone(),
            url: bookmark.original_url.clone(),
            domain: bookmark.domain.clone(),
            title: bookmark.title.clone(),
            text_content: bookmark.text.clone(),
            html_content: bookmark.html.clone(),
            images: bookmark
                .images
                .iter()
                .map(|i| i.original_url.clone())
                .collect(),
            created_at: Utc::now(),
        }
    }
}

#[instrument(skip_all)]
pub async fn run(
    pool: &Pool<Postgres>,
    http: &HttpClient,
    s3_client: &S3Client,
    config: &Config,
) -> Result<()> {
    loop {
        let tasks: Vec<Task> = peek_task(pool, Utc::now()).await?;
        if tasks.is_empty() {
            let duration = TokioDuration::from_secs(10);
            tracing::debug!(
                "No new task, going to sleep for {} seconds",
                &duration.as_secs()
            );
            sleep(duration).await;
            continue;
        } else {
            tracing::info!("New tasks found {}", tasks.len());
        }
        for task in tasks {
            tracing::info!(
                url = &task.url,
                task_id = format!("{}", &task.task_id),
                "Executing task"
            );
            match handle_task(pool, http, s3_client, config, &task).await {
                Ok(_) => {
                    tracing::info!("success");
                    update_task(pool, &task, TaskStatus::Done, None, None).await?;
                    tracing::info!(task_uuid = format!("{}", task.task_id), "Task executed")
                }
                Err(error) => {
                    if task.should_retry() {
                        tracing::info!("fail retry, reason = {:?}", &error);
                        update_task(
                            pool,
                            &task,
                            TaskStatus::Pending,
                            Some(task.retries.unwrap_or(0) + 1),
                            None,
                        )
                        .await?;
                        tracing::warn!(
                            task_uuid = format!("{}", task.task_id),
                            retries = &task.retries,
                            "Task failed, retying, error info: {}",
                            error
                        )
                    } else {
                        tracing::info!("fail");
                        update_task(
                            pool,
                            &task,
                            TaskStatus::Fail,
                            None,
                            Some(format!("{error}")),
                        )
                        .await?;
                        tracing::error!(
                            task_uuid = format!("{}", task.task_id),
                            "Task failed, error info: {error}"
                        )
                    }
                }
            }
        }
    }
}

#[instrument(skip(pool, http, s3_client, config))]
async fn handle_task(
    pool: &Pool<Postgres>,
    http: &HttpClient,
    s3_client: &S3Client,
    config: &Config,
    task: &Task,
) -> Result<()> {
    let bookmark =
        crease_or_retrieve_bookmark(pool, http, s3_client, config, &task.clean_url()?).await?;
    let uuid = save_user_bookmark(pool, &bookmark, task).await?;
    tracing::info!(
        user_id = format!("{}", task.user_id),
        bookmark_user_id = format!("{uuid}"),
        bookmark_id = &bookmark.bookmark_id,
        "Creating a new bookmark and bound with user",
    );
    Ok(())
}

#[instrument(skip(pool, http, s3_client, config))]
async fn crease_or_retrieve_bookmark(
    pool: &Pool<Postgres>,
    http: &HttpClient,
    s3_client: &S3Client,
    config: &Config,
    url: &str,
) -> Result<DatabaseBookmark> {
    match find_bookmark_by_url(pool, url).await? {
        Some(bookmark) => Ok(bookmark),
        None => {
            tracing::info!("Processing new bookmark for url={url}");
            let bookmark = processor::process_url(
                http,
                &config.readability_endpoint,
                url,
                &config.external_s3_endpoint,
                &config.s3_bucket,
            )
            .await
            .with_context(|| format!("process_url: {url}"))?;
            let stored_bookmark: DatabaseBookmark = (&bookmark).into();
            save_static_content(s3_client, config, &bookmark)
                .await
                .with_context(|| format!("save_static_content: bookmark_id={}", &bookmark.id))?;
            save_bookmark_into_database(pool, &stored_bookmark)
                .await
                .with_context(|| {
                    format!("save_bookmark_into_database: bookmark_id={}", &bookmark.id)
                })?;
            tracing::info!(
                url = url,
                bookmark_id = format!("{}", &bookmark.id),
                "Bookmark created",
            );
            Ok(stored_bookmark)
        }
    }
}

#[instrument(skip(pool))]
async fn peek_task(pool: &Pool<Postgres>, now: DateTime<Utc>) -> Result<Vec<Task>> {
    // https://softwaremill.com/mqperf/#postgresql
    let sql = r#"
    SELECT * FROM bookmark_task WHERE next_delivery <= $1 AND status = 'pending'
    FOR UPDATE SKIP LOCKED LIMIT 10
    "#;
    let mut tx = pool.begin().await?;
    let result: Vec<Task> = sqlx::query_as(sql).bind(now).fetch_all(&mut tx).await?;
    let ids: Vec<Uuid> = result.iter().map(|t| t.task_id).collect();
    let sql = "UPDATE bookmark_task SET next_delivery = $1 WHERE task_id = ANY ($2)";
    let next_delivery = now + Duration::minutes(5);
    let update_result = sqlx::query(sql)
        .bind(next_delivery)
        .bind(&ids)
        .execute(&mut tx)
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
async fn find_bookmark_by_url(
    pool: &Pool<Postgres>,
    url: &str,
) -> Result<Option<DatabaseBookmark>> {
    let sql = "SELECT * from bookmark WHERE url = $1";
    let result: Option<DatabaseBookmark> =
        sqlx::query_as(sql).bind(url).fetch_optional(pool).await?;
    Ok(result)
}

async fn save_static_content(
    s3_client: &S3Client,
    config: &Config,
    bookmark: &Bookmark,
) -> Result<()> {
    tracing::info!(
        "Saving bookmark, id={}, images={}",
        &bookmark.id,
        &bookmark.images.len()
    );
    for image in bookmark.images.iter() {
        let key = format!("{}/{}", bookmark.id, image.id);
        let put_response = s3_client
            .put_object()
            .bucket(&config.s3_bucket)
            .key(key)
            .content_type(&image.content_type)
            .body(ByteStream::from(image.bytes.to_vec()))
            .send()
            .await?;
        tracing::info!("Put response: {:?}", put_response);
    }
    Ok(())
}

#[instrument(skip(pool))]
async fn update_task(
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

#[instrument(skip(pool))]
async fn save_user_bookmark(
    pool: &Pool<Postgres>,
    bookmark: &DatabaseBookmark,
    task: &Task,
) -> Result<Uuid> {
    let sql = r#"
    INSERT INTO bookmark_user
    (bookmark_user_id, bookmark_id, user_id, tags, created_at, updated_at)
    VALUES (uuid_generate_v4(), $1, $2, $3, now(), now())
    ON CONFLICT ON CONSTRAINT bookmark_user_unique
    DO UPDATE SET tags = $3, updated_at = now()
    RETURNING bookmark_user_id
    "#;
    let result: Uuid = sqlx::query_scalar(sql)
        .bind(&bookmark.bookmark_id)
        .bind(task.user_id)
        .bind(&task.tags)
        .fetch_one(pool)
        .await?;
    Ok(result)
}

#[instrument(skip(pool))]
async fn save_bookmark_into_database(
    pool: &Pool<Postgres>,
    bookmark: &DatabaseBookmark,
) -> Result<()> {
    let sql = r#"
    INSERT INTO bookmark
    (bookmark_id, url, domain, title, text_content, html_content, images, created_at)
    VALUES ($1, $2, $3, $4, $5, $6, $7, now());
    "#;
    sqlx::query(sql)
        .bind(&bookmark.bookmark_id)
        .bind(&bookmark.url)
        .bind(&bookmark.domain)
        .bind(&bookmark.title)
        .bind(&bookmark.text_content)
        .bind(&bookmark.html_content)
        .bind(&bookmark.images)
        .execute(pool)
        .await?;
    Ok(())
}
