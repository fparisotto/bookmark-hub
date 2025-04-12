use chrono::{DateTime, Utc};
use deadpool_postgres::GenericClient;
use postgres_from_row::FromRow;
use serde::{Deserialize, Serialize};
use shared::{Bookmark, TagOperation};
use tracing::debug;
use uuid::Uuid;

use crate::error::{Error, Result};

use super::PgPool;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
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
        .map(|row| {
            RowBookmark::try_from_row(row)
                .map(Bookmark::from)
                .map_err(Error::from)
        })
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
        .map(|row| {
            RowBookmark::try_from_row(row)
                .map(Bookmark::from)
                .map_err(Error::from)
        })
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
        .map(|row| {
            RowBookmark::try_from_row(&row)
                .map(Bookmark::from)
                .map_err(Error::from)
        })
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
        .map(|row| {
            RowBookmark::try_from_row(&row)
                .map(Bookmark::from)
                .map_err(Error::from)
        })
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
    let result = RowBookmark::try_from_row(&row)
        .map(Bookmark::from)
        .map_err(Error::from)?;
    debug!(?operation, %bookmark_id, "Updated tags for bookmark");
    Ok(result)
}

pub async fn save(pool: &PgPool, bookmark: &Bookmark, text_content: &str) -> Result<Bookmark> {
    const SQL: &str = r#"
    INSERT INTO bookmark
    (bookmark_id, user_id, url, domain, title, text_content, tags, created_at, updated_at)
    VALUES ($1, $2, $3, $4, $5, $6, $7, now(), now()) RETURNING *;"#;
    let client = pool.get().await?;
    let row = client
        .query_one(
            SQL,
            &[
                &bookmark.bookmark_id,
                &bookmark.user_id,
                &bookmark.url,
                &bookmark.domain,
                &bookmark.title,
                &text_content,
                &bookmark.tags,
            ],
        )
        .await?;
    let result = RowBookmark::try_from_row(&row)
        .map(Bookmark::from)
        .map_err(Error::from)?;
    debug!(id = bookmark.bookmark_id, "Bookmark safe");
    Ok(result)
}

pub async fn update_summary(
    pool: &PgPool,
    user_id: Uuid,
    bookmark_id: &str,
    summary: &str,
) -> Result<Bookmark> {
    const SQL: &str = "UPDATE bookmark SET summary = $1, updated_at=now() WHERE bookmark_id=$2 AND user_id=$3 RETURNING *;";
    let client = pool.get().await?;
    let row = client
        .query_one(SQL, &[&summary, &bookmark_id, &user_id])
        .await?;
    let result = RowBookmark::try_from_row(&row)
        .map(Bookmark::from)
        .map_err(Error::from)?;
    debug!(%bookmark_id, "Updated summary for bookmark");
    Ok(result)
}

pub async fn get_text_content(
    pool: &PgPool,
    user_id: Uuid,
    bookmark_id: &str,
) -> Result<Option<String>> {
    const SQL: &str = "SELECT text_content FROM bookmark WHERE bookmark_id = $1 AND user_id = $2";
    let client = pool.get().await?;
    let result: Option<String> = client
        .query_opt(SQL, &[&bookmark_id, &user_id])
        .await?
        .map(|row| row.try_get(0))
        .transpose()?;
    Ok(result)
}

pub async fn get_untagged_bookmarks(pool: &PgPool, limit: usize) -> Result<Vec<Bookmark>> {
    let sql  =
        format!("SELECT * FROM bookmark WHERE tags IS NULL OR coalesce(array_length(tags, 1), 0) = 0 ORDER BY random() LIMIT {limit};");
    let client = pool.get().await?;
    let results = client
        .query(&sql, &[])
        .await?
        .iter()
        .map(|row| {
            RowBookmark::try_from_row(row)
                .map(Bookmark::from)
                .map_err(Error::from)
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(results)
}

pub async fn get_bookmarks_without_summary(
    pool: &PgPool,
    limit: usize,
) -> anyhow::Result<Vec<Bookmark>> {
    let sql =
        format!("SELECT * FROM bookmark WHERE summary IS NULL ORDER BY random() LIMIT {limit};");
    let client = pool.get().await?;
    let results = client
        .query(&sql, &[])
        .await?
        .iter()
        .map(|row| {
            RowBookmark::try_from_row(row)
                .map(Bookmark::from)
                .map_err(Error::from)
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(results)
}
