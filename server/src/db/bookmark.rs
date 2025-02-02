use chrono::{DateTime, Utc};
use deadpool_postgres::GenericClient;
use postgres_from_row::FromRow;
use serde::{Deserialize, Serialize};
use shared::{Bookmark, TagOperation};
use tracing::{info, instrument};
use uuid::Uuid;

use crate::error::{Error, Result};

use super::PgPool;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
struct RowBookmark {
    bookmark_id: String,
    url: String,
    domain: String,
    title: String,
    created_at: DateTime<Utc>,
    user_id: Option<Uuid>,
    tags: Option<Vec<String>>,
    user_created_at: Option<DateTime<Utc>>,
    user_updated_at: Option<DateTime<Utc>>,
}

impl From<RowBookmark> for Bookmark {
    fn from(value: RowBookmark) -> Self {
        Self {
            bookmark_id: value.bookmark_id,
            url: value.url,
            domain: value.domain,
            title: value.title,
            links: None,
            created_at: value.created_at,
            user_id: value.user_id,
            tags: value.tags,
            user_created_at: value.user_created_at.unwrap_or_default(),
            user_updated_at: value.user_updated_at,
        }
    }
}

#[instrument(skip(pool))]
pub async fn get_tag_count_by_user(pool: &PgPool, user_id: Uuid) -> Result<Vec<(String, i64)>> {
    const SQL: &str = r#"
    WITH tags AS (
        SELECT unnest(tags) AS tag
        FROM bookmark_user
        WHERE user_id = $1
    )
    SELECT tag, count(1) AS counter FROM tags GROUP BY tag;"#;
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

#[instrument(skip(pool))]
pub async fn get_by_user(pool: &PgPool, user_id: Uuid) -> Result<Vec<Bookmark>> {
    const SQL: &str = r#"
    SELECT
        b.*,
        bu.user_id,
        bu.tags,
        bu.created_at as user_created_at,
        bu.updated_at as user_updated_at
    FROM bookmark_user bu
    INNER JOIN bookmark b USING(bookmark_id)
    WHERE bu.user_id = $1
    ORDER BY bu.created_at ASC;"#;
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

#[instrument(skip(pool))]
pub async fn get_by_tag(pool: &PgPool, user_id: Uuid, tag: &str) -> Result<Vec<Bookmark>> {
    const SQL: &str = r#"
    SELECT
        b.*,
        bu.user_id,
        bu.tags,
        bu.created_at as user_created_at,
        bu.updated_at as user_updated_at
    FROM bookmark_user bu
    INNER JOIN bookmark b USING(bookmark_id)
    WHERE bu.user_id = $1
    AND bu.tags @> $2
    ORDER BY bu.created_at ASC;"#;
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

#[instrument(skip(pool))]
pub async fn get_by_url(pool: &PgPool, url: &str) -> Result<Option<Bookmark>> {
    const SQL: &str = "SELECT * FROM bookmark WHERE url = $1;";
    let client = pool.get().await?;
    let result = client
        .query_opt(SQL, &[&url])
        .await?
        .map(|row| {
            RowBookmark::try_from_row(&row)
                .map(Bookmark::from)
                .map_err(Error::from)
        })
        .transpose()?;
    Ok(result)
}

#[instrument(skip(pool))]
pub async fn get_with_user_data(
    pool: &PgPool,
    user_id: Uuid,
    bookmark_id: &str,
) -> Result<Option<Bookmark>> {
    const SQL: &str = r#"
    SELECT
        b.*,
        bu.user_id,
        bu.tags,
        bu.created_at as user_created_at,
        bu.updated_at as user_updated_at
    FROM bookmark_user bu
    INNER JOIN bookmark b USING(bookmark_id)
    WHERE bu.user_id = $1
    AND bookmark_id = $2;"#;
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

#[instrument(skip(pool))]
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
        r#"
        WITH update_bookmark_user AS (
            UPDATE bookmark_user
            SET {update_tag_sql}, updated_at=now()
            WHERE bookmark_id=$2 AND user_id=$3
            RETURNING *
        )
        SELECT
            b.*,
            bi.user_id,
            bi.tags,
            bi.created_at as user_created_at,
            bi.updated_at as user_updated_at
        FROM update_bookmark_user bi
        INNER JOIN bookmark b using(bookmark_id);"#
    );
    let client = pool.get().await?;
    let row = client
        .query_one(&sql, &[&tags, &bookmark_id, &user_id])
        .await?;
    let result = RowBookmark::try_from_row(&row)?;
    info!(?operation, %bookmark_id, "Updated tags for bookmark");
    Ok(result.into())
}

#[instrument(skip(pool))]
pub async fn upsert_user_bookmark(
    pool: &PgPool,
    bookmark_id: &str,
    user_id: Uuid,
    tags: &[String],
) -> Result<Uuid> {
    const SQL: &str = r#"
    INSERT INTO bookmark_user
    (bookmark_user_id, bookmark_id, user_id, tags, created_at, updated_at)
    VALUES (uuid_generate_v4(), $1, $2, $3, now(), now())
    ON CONFLICT ON CONSTRAINT bookmark_user_unique
    DO UPDATE SET tags = $3, updated_at = now()
    RETURNING bookmark_user_id;"#;
    let client = pool.get().await?;
    let row = client
        .query_one(SQL, &[&bookmark_id, &user_id, &tags])
        .await?;
    let uuid: Uuid = row.try_get(0)?;
    info!(?uuid, %bookmark_id, %user_id, ?tags, "Bookmark upsert");
    Ok(uuid)
}

#[instrument(skip(pool))]
pub async fn save(pool: &PgPool, bookmark: &Bookmark, content: &str) -> Result<()> {
    const SQL: &str = r#"
    INSERT INTO bookmark
    (bookmark_id, url, domain, title, text_content, created_at)
    VALUES ($1, $2, $3, $4, $5, now());"#;
    let client = pool.get().await?;
    let rows_affected = client
        .execute(
            SQL,
            &[
                &bookmark.bookmark_id,
                &bookmark.url,
                &bookmark.domain,
                &bookmark.title,
                &content,
            ],
        )
        .await?;
    info!(%rows_affected, ?bookmark, "Bookmark safe");
    Ok(())
}
