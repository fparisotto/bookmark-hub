use chrono::{DateTime, Utc};
use deadpool_postgres::GenericClient;
use pgvector::Vector;
use postgres_from_row::FromRow;
use serde::{Deserialize, Serialize};
use tracing::debug;
use uuid::Uuid;

use crate::error::{Error, Result};

use super::PgPool;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Bookmark {
    pub bookmark_id: String,
    pub user_id: Uuid,
    pub url: String,
    pub domain: String,
    pub title: String,
    pub text_content: String,
    pub tags: Vec<String>,
    pub summary: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub enum TagOperation {
    Set(Vec<String>),
    Append(Vec<String>),
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
    Ok(result)
}

pub async fn get_by_user(pool: &PgPool, user_id: Uuid) -> Result<Vec<Bookmark>> {
    const SQL: &str = "SELECT * FROM bookmark b WHERE b.user_id = $1 ORDER BY b.created_at ASC;";
    let client = pool.get().await?;
    let results = client
        .query(SQL, &[&user_id])
        .await?
        .iter()
        .map(|row| Bookmark::try_from_row(row).map_err(Error::from))
        .collect::<Result<Vec<_>>>()?;
    Ok(results)
}

pub async fn get_by_tag(pool: &PgPool, user_id: Uuid, tag: &str) -> Result<Vec<Bookmark>> {
    const SQL: &str =
        "SELECT * FROM bookmark b WHERE b.user_id = $1 AND b.tags @> $2 ORDER BY b.created_at ASC;";
    let client = pool.get().await?;
    let results = client
        .query(SQL, &[&user_id, &[&tag]])
        .await?
        .iter()
        .map(|row| Bookmark::try_from_row(row).map_err(Error::from))
        .collect::<Result<Vec<_>>>()?;
    Ok(results)
}

pub async fn get_by_url_and_user_id(
    pool: &PgPool,
    url: &str,
    user_id: Uuid,
) -> Result<Option<Bookmark>> {
    const SQL: &str = "SELECT * FROM bookmark WHERE url = $1 AND user_id = $2;";
    let client = pool.get().await?;
    let result = client
        .query_opt(SQL, &[&url, &user_id])
        .await?
        .map(|row| Bookmark::try_from_row(&row).map_err(Error::from))
        .transpose()?;
    Ok(result)
}

pub async fn get_with_user_data(
    pool: &PgPool,
    user_id: Uuid,
    bookmark_id: &str,
) -> Result<Option<Bookmark>> {
    const SQL: &str = "SELECT * FROM bookmark b WHERE b.user_id = $1 AND b.bookmark_id = $2;";
    let client = pool.get().await?;
    let result = client
        .query_opt(SQL, &[&user_id, &bookmark_id])
        .await?
        .map(|row| Bookmark::try_from_row(&row).map_err(Error::from))
        .transpose()?;
    Ok(result)
}

pub async fn update_tags(
    pool: &PgPool,
    user_id: Uuid,
    bookmark_id: &str,
    operation: &TagOperation,
) -> Result<Bookmark> {
    let (update_tag_sql, tags) = match operation.clone() {
        TagOperation::Set(tags) => ("tags=$1", tags),
        TagOperation::Append(tags) => ("tags=array_cat(tags, $1)", tags),
    };
    let sql = format!(
        "UPDATE bookmark SET {update_tag_sql}, updated_at=now() WHERE bookmark_id=$2 AND user_id=$3 RETURNING *;"
    );
    let client = pool.get().await?;
    let row = client
        .query_one(&sql, &[&tags, &bookmark_id, &user_id])
        .await?;
    let result = Bookmark::try_from_row(&row)?;
    debug!(?operation, %bookmark_id, "Updated tags for bookmark");
    Ok(result)
}

pub async fn save(pool: &PgPool, bookmark: &Bookmark, embedding: Vec<f32>) -> Result<Bookmark> {
    const SQL: &str = r#"
    INSERT INTO bookmark
    (bookmark_id, user_id, url, domain, title, text_content, tags, summary, embedding, created_at, updated_at)
    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, now(), now()) RETURNING *;"#;
    let client = pool.get().await?;
    let embeddings = Vector::from(embedding);
    let row = client
        .query_one(
            SQL,
            &[
                &bookmark.bookmark_id,
                &bookmark.user_id,
                &bookmark.url,
                &bookmark.domain,
                &bookmark.title,
                &bookmark.text_content,
                &bookmark.tags,
                &bookmark.summary,
                &embeddings,
            ],
        )
        .await?;
    let result = Bookmark::try_from_row(&row)?;
    debug!(id = bookmark.bookmark_id, "Bookmark safe");
    Ok(result)
}
