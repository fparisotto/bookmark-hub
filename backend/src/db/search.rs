use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres, QueryBuilder};
use tokio::try_join;
use tracing::instrument;
use uuid::Uuid;

use crate::error::Result;

#[derive(Debug, sqlx::FromRow, Serialize)]
struct TagCount {
    tag: String,
    count: i64,
}

#[derive(Serialize)]
pub struct SearchResponse {
    bookmarks: Vec<SearchResultItem>,
    tags: Vec<TagCount>,
    total: u64,
}

#[derive(Debug, sqlx::FromRow, Serialize, Deserialize)]
pub struct SearchResultItem {
    pub bookmark_id: String,
    pub url: String,
    pub domain: String,
    pub title: String,
    pub search_match: Option<String>,
    pub created_at: DateTime<Utc>,
    pub user_id: Option<Uuid>,
    pub tags: Option<Vec<String>>,
    pub user_created_at: Option<DateTime<Utc>>,
    pub user_updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Deserialize)]
pub enum TagFilter {
    And(Vec<String>),
    Or(Vec<String>),
    Any,
    Untagged,
}

#[derive(Debug, Deserialize)]
pub struct SearchRequest {
    query: Option<String>,
    tags_filter: Option<TagFilter>,
    limit: Option<i32>,
}

#[instrument(skip(db))]
pub async fn search(
    db: &Pool<Postgres>,
    user_id: &Uuid,
    request: SearchRequest,
) -> Result<SearchResponse> {
    // TODO use a tx
    let f_search = run_search(db, user_id, &request);
    let f_aggregation = run_aggregation(db, user_id, &request);
    let f_total = run_total(db, user_id, &request);
    let (bookmarks, tags, total) = try_join!(f_search, f_aggregation, f_total)?;
    Ok(SearchResponse {
        bookmarks,
        tags,
        total,
    })
}

#[instrument(skip(db))]
async fn run_total(db: &Pool<Postgres>, user_id: &Uuid, request: &SearchRequest) -> Result<u64> {
    let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new("");
    query_builder.push(
        r#"
            select
                count(1)
            from
                bookmark_user bu
            inner join
                bookmark b using(bookmark_id)
            "#,
    );
    query_builder
        .push(" where bu.user_id = ")
        .push_bind(user_id);
    match request.tags_filter.clone().unwrap_or(TagFilter::Any) {
        TagFilter::And(tags) => {
            query_builder.push(" and bu.tags @> ").push_bind(tags);
        }
        TagFilter::Or(tags) => {
            query_builder.push(" and bu.tags && ").push_bind(tags);
        }
        TagFilter::Untagged => {
            query_builder.push(" and cardinality(bu.tags) = 0 ");
        }
        TagFilter::Any => (),
    }
    if let Some(query) = &request.query {
        if query.trim().starts_with('"') && query.trim().ends_with('"') {
            query_builder
                .push(" and b.search_tokens @@ phraseto_tsquery('english', ")
                .push_bind(query)
                .push(" )");
        } else {
            let query = if query.contains('&') {
                query.to_owned()
            } else {
                query.split(' ').collect::<Vec<_>>().join(" | ")
            };
            query_builder
                .push(" and b.search_tokens @@ to_tsquery('english', ")
                .push_bind(query.clone())
                .push(" )");
        }
    }
    tracing::debug!("Total query {}", &query_builder.sql());
    let (total,): (i64,) = query_builder.build_query_as().fetch_one(db).await?;
    Ok(total as u64)
}

#[instrument(skip(db))]
async fn run_search(
    db: &Pool<Postgres>,
    user_id: &Uuid,
    request: &SearchRequest,
) -> Result<Vec<SearchResultItem>> {
    let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new("");
    query_builder.push("select ");
    if let Some(query) = &request.query {
        if query.trim().starts_with('"') && query.trim().ends_with('"') {
            query_builder
                .push(" ts_headline('english', b.text_content, phraseto_tsquery('english', ")
                .push_bind(query)
                .push(" ), 'StartSel=<mark>, StopSel=</mark>') as search_match, ");
        } else {
            let query = if query.contains('&') {
                query.to_owned()
            } else {
                query.split(' ').collect::<Vec<_>>().join(" | ")
            };
            query_builder
                .push(" ts_headline('english', b.text_content, to_tsquery('english', ")
                .push_bind(query)
                .push(" ), 'StartSel=<mark>, StopSel=</mark>') as search_match, ");
        }
    } else {
        query_builder.push(" null as search_match, ");
    }
    query_builder.push(
        r#"
                b.*,
                bu.user_id,
                bu.tags,
                bu.created_at as user_created_at,
                bu.updated_at as user_updated_at
            from
                bookmark_user bu
            inner join
                bookmark b using(bookmark_id)
            "#,
    );
    query_builder
        .push(" where bu.user_id = ")
        .push_bind(user_id);
    match request.tags_filter.clone().unwrap_or(TagFilter::Any) {
        TagFilter::And(tags) => {
            query_builder.push(" and bu.tags @> ").push_bind(tags);
        }
        TagFilter::Or(tags) => {
            query_builder.push(" and bu.tags && ").push_bind(tags);
        }
        TagFilter::Untagged => {
            query_builder.push(" and cardinality(bu.tags) = 0 ");
        }
        TagFilter::Any => (),
    }
    if let Some(query) = &request.query {
        if query.trim().starts_with('"') && query.trim().ends_with('"') {
            query_builder
                .push(" and b.search_tokens @@ phraseto_tsquery('english', ")
                .push_bind(query)
                .push(" ) order by ts_rank(b.search_tokens, phraseto_tsquery('english', ")
                .push_bind(query)
                .push(" )) ");
        } else {
            let query = if query.contains('&') {
                query.to_owned()
            } else {
                query.split(' ').collect::<Vec<_>>().join(" | ")
            };
            query_builder
                .push(" and b.search_tokens @@ to_tsquery('english', ")
                .push_bind(query.clone())
                .push(" ) order by ts_rank(b.search_tokens, to_tsquery('english', ")
                .push_bind(query.clone())
                .push(" )) ");
        }
    }
    let limit_value = request.limit.unwrap_or(20);
    query_builder.push(" limit ").push_bind(limit_value);
    tracing::debug!("Search query {}", &query_builder.sql());
    let bookmarks: Vec<SearchResultItem> = query_builder.build_query_as().fetch_all(db).await?;
    Ok(bookmarks)
}

#[instrument(skip(db))]
async fn run_aggregation(
    db: &Pool<Postgres>,
    user_id: &Uuid,
    request: &SearchRequest,
) -> Result<Vec<TagCount>> {
    let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new("");
    query_builder.push(
        r#"
            with tags as (
                select
                    unnest(bu.tags) as tag
                from
                    bookmark_user bu
                inner join
                    bookmark b using(bookmark_id)
            "#,
    );
    query_builder
        .push(" where bu.user_id = ")
        .push_bind(user_id);
    match request.tags_filter.clone().unwrap_or(TagFilter::Any) {
        TagFilter::And(tags) => {
            query_builder.push(" and bu.tags @> ").push_bind(tags);
        }
        TagFilter::Or(tags) => {
            query_builder.push(" and bu.tags && ").push_bind(tags);
        }
        TagFilter::Untagged => {
            query_builder.push(" and cardinality(bu.tags) = 0 ");
        }
        TagFilter::Any => (),
    }
    if let Some(query) = &request.query {
        if query.trim().starts_with('"') && query.trim().ends_with('"') {
            query_builder
                .push(" and b.search_tokens @@ phraseto_tsquery('english', ")
                .push_bind(query)
                .push(" ) order by ts_rank(b.search_tokens, phraseto_tsquery('english', ")
                .push_bind(query)
                .push(" )) ");
        } else {
            let query = if query.contains('&') {
                query.to_owned()
            } else {
                query.split(' ').collect::<Vec<_>>().join(" | ")
            };
            query_builder
                .push(" and b.search_tokens @@ to_tsquery('english', ")
                .push_bind(query.clone())
                .push(" ) order by ts_rank(b.search_tokens, to_tsquery('english', ")
                .push_bind(query.clone())
                .push(" )) ");
        }
    }
    let sql = r#"
            )
            select
                tag,
                count(1) as count
            from
                tags t
            group by
                tag
        "#;
    query_builder.push(sql);
    tracing::debug!("Aggregation query {}", &query_builder.sql());
    let tags: Vec<TagCount> = query_builder.build_query_as().fetch_all(db).await?;
    Ok(tags)
}
