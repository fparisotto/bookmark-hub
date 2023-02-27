use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres, Transaction};
use uuid::Uuid;

use crate::error::Result;

#[derive(Debug, sqlx::FromRow, Serialize, Deserialize)]
pub struct BookmarkWithUserData {
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

pub enum TagOperation {
    Set(Vec<String>),
    Append(Vec<String>),
}

pub enum SearchType {
    Query(String),
    Phrase(String),
}

pub struct BookmarkTable;

impl BookmarkTable {
    pub async fn get_tag_count_by_user(
        db: &Pool<Postgres>,
        user_id: &Uuid,
    ) -> Result<Vec<(String, i64)>> {
        let sql = r#"
        with tags as (
            select unnest(tags) as tag
            from bookmark_user
            where user_id = $1
        )
        select tag, count(1) as counter from tags group by tag
        "#;
        let result: Vec<(String, i64)> = sqlx::query_as(sql).bind(user_id).fetch_all(db).await?;
        Ok(result)
    }

    pub async fn get_bookmarks_by_user(
        db: &Pool<Postgres>,
        user_id: &Uuid,
    ) -> Result<Vec<BookmarkWithUserData>> {
        let sql = r#"
        select
            b.*,
            bu.user_id,
            bu.tags,
            bu.created_at as user_created_at,
            bu.updated_at as user_updated_at
        from bookmark_user bu
        inner join bookmark b using(bookmark_id)
        where bu.user_id = $1
        order by bu.created_at asc;
        "#;
        let result: Vec<BookmarkWithUserData> =
            sqlx::query_as(sql).bind(user_id).fetch_all(db).await?;
        Ok(result)
    }

    pub async fn get_bookmarks_by_tag(
        db: &Pool<Postgres>,
        user_id: &Uuid,
        tag: &str,
    ) -> Result<Vec<BookmarkWithUserData>> {
        let sql = r#"
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
        order by bu.created_at asc;
        "#;
        let tags: Vec<String> = vec![tag.to_string()];
        let result: Vec<BookmarkWithUserData> = sqlx::query_as(sql)
            .bind(user_id)
            .bind(tags)
            .fetch_all(db)
            .await?;
        Ok(result)
    }

    pub async fn get_bookmark_with_user_data(
        db: &Pool<Postgres>,
        user_id: &Uuid,
        bookmark_id: &String,
    ) -> Result<Option<BookmarkWithUserData>> {
        let sql = r#"
        select 
            b.*, 
            bu.user_id, 
            bu.tags, 
            bu.created_at as user_created_at,
            bu.updated_at as user_updated_at
        from bookmark_user bu 
        inner join bookmark b using(bookmark_id) 
        where bu.user_id = $1
        and bookmark_id = $2;
        "#;
        let result: Option<BookmarkWithUserData> = sqlx::query_as(sql)
            .bind(user_id)
            .bind(bookmark_id)
            .fetch_optional(db)
            .await?;
        Ok(result)
    }

    pub async fn update_tags(
        tx: &mut Transaction<'_, Postgres>,
        user_id: &Uuid,
        bookmark_id: &String,
        operation: TagOperation,
    ) -> Result<BookmarkWithUserData> {
        let (update_tag_sql, tags) = match operation {
            TagOperation::Set(tags) => ("tags=$1", tags),
            TagOperation::Append(tags) => ("tags=array_cat(tags, $1)", tags),
        };
        let sql = format!(
            r#"
        with update_bookmark_user as (
            update public.bookmark_user
            set {0}, updated_at=now()
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
        inner join bookmark b using(bookmark_id)
        "#,
            update_tag_sql
        );
        let result: BookmarkWithUserData = sqlx::query_as(&sql)
            .bind(tags)
            .bind(bookmark_id)
            .bind(user_id)
            .fetch_one(tx)
            .await?;
        Ok(result)
    }
}
