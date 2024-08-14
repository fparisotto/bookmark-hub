use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};
use tracing::instrument;
use uuid::Uuid;

use crate::error::Result;

#[derive(Debug, sqlx::FromRow, Serialize, Deserialize)]
struct Bookmark {
    pub bookmark_id: String,
    pub url: String,
    pub domain: String,
    pub title: String,
    pub text_content: String,
    pub html_content: String,
    pub images: Vec<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow, Serialize, Deserialize)]
pub struct BookmarkWithUser {
    pub bookmark_id: String,
    pub url: String,
    pub domain: String,
    pub title: String,
    pub html_content: String,
    pub links: Option<Vec<String>>,
    pub created_at: DateTime<Utc>,
    pub user_id: Option<Uuid>,
    pub tags: Option<Vec<String>>,
    pub user_created_at: Option<DateTime<Utc>>,
    pub user_updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug)]
pub enum TagOperation {
    Set(Vec<String>),
    Append(Vec<String>),
}

#[instrument(skip(db))]
pub async fn get_tag_count_by_user(
    db: &Pool<Postgres>,
    user_id: &Uuid,
) -> Result<Vec<(String, i64)>> {
    const SQL: &str = r#"
    with tags as (
        select unnest(tags) as tag
        from bookmark_user
        where user_id = $1
    )
    select tag, count(1) as counter from tags group by tag;"#;
    let result: Vec<(String, i64)> = sqlx::query_as(SQL).bind(user_id).fetch_all(db).await?;
    Ok(result)
}

#[instrument(skip(db))]
pub async fn get_by_user(db: &Pool<Postgres>, user_id: &Uuid) -> Result<Vec<BookmarkWithUser>> {
    const SQL: &str = r#"
    select
        b.*,
        bu.user_id,
        bu.tags,
        bu.created_at as user_created_at,
        bu.updated_at as user_updated_at
    from bookmark_user bu
    inner join bookmark b using(bookmark_id)
    where bu.user_id = $1
    order by bu.created_at asc;"#;
    let result: Vec<BookmarkWithUser> = sqlx::query_as(SQL).bind(user_id).fetch_all(db).await?;
    Ok(result)
}

#[instrument(skip(db))]
pub async fn get_by_tag(
    db: &Pool<Postgres>,
    user_id: &Uuid,
    tag: &str,
) -> Result<Vec<BookmarkWithUser>> {
    const SQL: &str = r#"
    select
        b.*,
        bu.user_id,
        bu.tags,
        bu.created_at as user_created_at,
        bu.updated_at as user_updated_at
    from bookmark_user bu
    inner join bookmark b using(bookmark_id)
    where bu.user_id = $1
    and bu.tags @> $2
    order by bu.created_at asc;"#;
    let tags: Vec<String> = vec![tag.to_string()];
    let result: Vec<BookmarkWithUser> = sqlx::query_as(SQL)
        .bind(user_id)
        .bind(tags)
        .fetch_all(db)
        .await?;
    Ok(result)
}

#[instrument(skip(pool))]
async fn get_by_url(pool: &Pool<Postgres>, url: &str) -> Result<Option<Bookmark>> {
    const SQL: &str = "SELECT * from bookmark WHERE url = $1";
    let result: Option<Bookmark> = sqlx::query_as(SQL).bind(url).fetch_optional(pool).await?;
    Ok(result)
}

#[instrument(skip(db))]
pub async fn get_with_user_data(
    db: &Pool<Postgres>,
    user_id: &Uuid,
    bookmark_id: &String,
) -> Result<Option<BookmarkWithUser>> {
    const SQL: &str = r#"
    select
        b.*,
        bu.user_id,
        bu.tags,
        bu.created_at as user_created_at,
        bu.updated_at as user_updated_at
    from bookmark_user bu
    inner join bookmark b using(bookmark_id)
    where bu.user_id = $1
    and bookmark_id = $2;"#;
    let result: Option<BookmarkWithUser> = sqlx::query_as(SQL)
        .bind(user_id)
        .bind(bookmark_id)
        .fetch_optional(db)
        .await?;
    Ok(result)
}

#[instrument(skip(db))]
pub async fn update_tags(
    db: &Pool<Postgres>,
    user_id: &Uuid,
    bookmark_id: &String,
    operation: TagOperation,
) -> Result<BookmarkWithUser> {
    let (update_tag_sql, tags) = match operation {
        TagOperation::Set(tags) => ("tags=$1", tags),
        TagOperation::Append(tags) => ("tags=array_cat(tags, $1)", tags),
    };
    let sql = format!(
        r#"
        with update_bookmark_user as (
            update public.bookmark_user
            set {update_tag_sql}, updated_at=now()
            where bookmark_id=$2 and user_id=$3
            returning *
        )
        select
            b.*,
            bi.user_id,
            bi.tags,
            bi.created_at as user_created_at,
            bi.updated_at as user_updated_at
        from update_bookmark_user bi
        inner join bookmark b using(bookmark_id);"#
    );
    let result: BookmarkWithUser = sqlx::query_as(&sql)
        .bind(tags)
        .bind(bookmark_id)
        .bind(user_id)
        .fetch_one(db)
        .await?;
    Ok(result)
}

#[instrument(skip(pool))]
async fn upsert_user_bookmark(
    pool: &Pool<Postgres>,
    bookmark_id: String,
    user_id: Uuid,
    tags: Vec<String>,
) -> Result<Uuid> {
    const SQL: &str = r#"
    INSERT INTO bookmark_user
    (bookmark_user_id, bookmark_id, user_id, tags, created_at, updated_at)
    VALUES (uuid_generate_v4(), $1, $2, $3, now(), now())
    ON CONFLICT ON CONSTRAINT bookmark_user_unique
    DO UPDATE SET tags = $3, updated_at = now()
    RETURNING bookmark_user_id;"#;
    let result: Uuid = sqlx::query_scalar(SQL)
        .bind(bookmark_id)
        .bind(user_id)
        .bind(tags)
        .fetch_one(pool)
        .await?;
    Ok(result)
}

#[instrument(skip(pool))]
async fn save(pool: &Pool<Postgres>, bookmark: &Bookmark) -> Result<()> {
    const SQL: &str = r#"
    INSERT INTO bookmark
    (bookmark_id, url, domain, title, text_content, html_content, images, created_at)
    VALUES ($1, $2, $3, $4, $5, $6, $7, now());"#;
    sqlx::query(SQL)
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
