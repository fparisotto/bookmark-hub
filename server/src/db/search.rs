use chrono::{DateTime, Utc};
use deadpool_postgres::GenericClient;
use futures::TryFutureExt;
use postgres_from_row::FromRow;
use serde::{Deserialize, Serialize};
use shared::{Bookmark, SearchRequest, SearchResponse, SearchResultItem, TagCount, TagFilter};
use tokio::try_join;
use tracing::{debug, warn};
use uuid::Uuid;

use crate::error::{Error, Result};

use super::{PgConnection, PgPool};

#[derive(Debug, Serialize, Deserialize, FromRow)]
struct RowTagCount {
    tag: String,
    count: i64,
}

impl From<RowTagCount> for TagCount {
    fn from(value: RowTagCount) -> Self {
        Self {
            tag: value.tag,
            count: value.count,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
struct RowSearchResultItem {
    bookmark_id: String,
    user_id: Uuid,
    url: String,
    domain: String,
    title: String,
    search_match: Option<String>,
    tags: Option<Vec<String>>,
    summary: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: Option<DateTime<Utc>>,
}

impl From<RowSearchResultItem> for Bookmark {
    fn from(value: RowSearchResultItem) -> Self {
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

impl From<RowSearchResultItem> for SearchResultItem {
    fn from(value: RowSearchResultItem) -> Self {
        Self {
            search_match: value.search_match.clone(),
            bookmark: value.into(),
        }
    }
}

pub async fn search(
    pool: &PgPool,
    user_id: Uuid,
    request: &SearchRequest,
) -> Result<SearchResponse> {
    // FIXME: run all queries on the same transaction
    let client = pool.get().await?;
    let f_search = run_search(&client, user_id, request).map_err(|e| {
        warn!("Search query fail");
        e
    });
    let f_aggregation = run_aggregation(&client, user_id, request).map_err(|e| {
        warn!("Aggregation query fail");
        e
    });
    let f_total = run_total(&client, user_id, request).map_err(|e| {
        warn!("Total query fail");
        e
    });
    let (items, tags, total) = try_join!(f_search, f_aggregation, f_total)?;
    Ok(SearchResponse { items, tags, total })
}

async fn run_total(client: &PgConnection, user_id: Uuid, request: &SearchRequest) -> Result<u64> {
    let sql: String = "SELECT COUNT(1) FROM bookmark b WHERE b.user_id = $1".to_owned();
    let (sql, query, tags) = modify_query_and_get_bindings(sql, request);
    debug!(?sql, ?query, ?tags, "Total query");
    match (query, tags) {
        (None, None) => {
            let row = client.query_one(&sql, &[&user_id]).await?;
            let total: i64 = row.try_get(0).map_err(Error::from)?;
            Ok(total as u64)
        }
        (None, Some(tags)) => {
            let row = client.query_one(&sql, &[&user_id, &tags]).await?;
            let total: i64 = row.try_get(0).map_err(Error::from)?;
            Ok(total as u64)
        }
        (Some(query), None) => {
            let row = client.query_one(&sql, &[&user_id, &query]).await?;
            let total: i64 = row.try_get(0).map_err(Error::from)?;
            Ok(total as u64)
        }
        (Some(query), Some(tags)) => {
            let row = client.query_one(&sql, &[&user_id, &query, &tags]).await?;
            let total: i64 = row.try_get(0).map_err(Error::from)?;
            Ok(total as u64)
        }
    }
}

async fn run_aggregation(
    client: &PgConnection,
    user_id: Uuid,
    request: &SearchRequest,
) -> Result<Vec<TagCount>> {
    let sql = "WITH tags AS (
        SELECT unnest(b.tags) AS tag FROM bookmark b WHERE b.user_id = $1 "
        .to_owned();
    let (mut sql, query, tags) = modify_query_and_get_bindings(sql, request);
    sql.push_str(" ) SELECT tag, count(1) AS count FROM tags t GROUP BY tag");
    debug!(%sql, ?query, ?tags, "Aggregation query");
    match (query, tags) {
        (None, None) => {
            let result = client
                .query(&sql, &[&user_id])
                .await?
                .iter()
                .map(|row| {
                    RowTagCount::try_from_row(row)
                        .map(TagCount::from)
                        .map_err(Error::from)
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(result)
        }
        (None, Some(tags)) => {
            let result = client
                .query(&sql, &[&user_id, &tags])
                .await?
                .iter()
                .map(|row| {
                    RowTagCount::try_from_row(row)
                        .map(TagCount::from)
                        .map_err(Error::from)
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(result)
        }
        (Some(query), None) => {
            let result = client
                .query(&sql, &[&user_id, &query])
                .await?
                .iter()
                .map(|row| {
                    RowTagCount::try_from_row(row)
                        .map(TagCount::from)
                        .map_err(Error::from)
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(result)
        }
        (Some(query), Some(tags)) => {
            let result = client
                .query(&sql, &[&user_id, &query, &tags])
                .await?
                .iter()
                .map(|row| {
                    RowTagCount::try_from_row(row)
                        .map(TagCount::from)
                        .map_err(Error::from)
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(result)
        }
    }
}

async fn run_search(
    client: &PgConnection,
    user_id: Uuid,
    request: &SearchRequest,
) -> Result<Vec<SearchResultItem>> {
    let mut sql = "SELECT ".to_owned();
    let query_select = match request.query.clone() {
        Some(query) => {
            sql.push_str(" ts_headline('english', b.text_content, websearch_to_tsquery('english', $1), 'StartSel=<mark>, StopSel=</mark>') AS search_match, ");
            Some(query)
        }
        None => {
            sql.push_str(" $1 AS search_match, ");
            None
        }
    };

    sql.push_str(
        " b.*, b.user_id, b.tags, b.created_at AS user_created_at, b.updated_at AS user_updated_at",
    );
    sql.push_str(" FROM bookmark b WHERE b.user_id = $2 ");

    let tags = match request.tags_filter.clone().unwrap_or(TagFilter::Any) {
        TagFilter::And(tags) => {
            sql.push_str(" AND b.tags @> $3 ");
            Some(tags)
        }
        TagFilter::Or(tags) => {
            sql.push_str(" AND b.tags && $3 ");
            Some(tags)
        }
        TagFilter::Untagged => {
            sql.push_str(" AND cardinality(b.tags) = 0 AND $3 IS NULL ");
            None
        }
        TagFilter::Any => {
            sql.push_str(" AND CAST($3 AS TEXT[]) IS NULL ");
            None
        }
    };

    let query_filter: Option<String> = match request.query.clone() {
        Some(query) => {
            sql.push_str(" AND b.search_tokens @@ websearch_to_tsquery('english', $4) ");
            sql.push_str(
                " ORDER BY ts_rank(b.search_tokens, websearch_to_tsquery('english', $4)) ",
            );
            Some(query)
        }
        None => {
            sql.push_str(" AND CAST($4 AS TEXT) IS NULL ");
            None
        }
    };

    let limit = format!(" LIMIT {} ", request.limit.unwrap_or(20));
    sql.push_str(&limit);

    debug!(
        ?sql,
        ?query_select,
        %user_id,
        ?tags,
        ?query_filter,
        "Search query"
    );

    let result = client
        .query(&sql, &[&query_select, &user_id, &tags, &query_filter])
        .await?
        .iter()
        .map(|row| {
            RowSearchResultItem::try_from_row(row)
                .map(SearchResultItem::from)
                .map_err(Error::from)
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(result)
}

fn modify_query_and_get_bindings(
    mut sql: String,
    request: &SearchRequest,
) -> (String, Option<String>, Option<Vec<String>>) {
    match (request.tags_filter.clone(), request.query.clone()) {
        (None, None) => (sql, None, None),
        (None, Some(query)) => {
            sql.push_str(" AND b.search_tokens @@ websearch_to_tsquery('english', $2) ");
            (sql, Some(query), None)
        }
        (Some(tag_filter), None) => {
            let tags = match tag_filter {
                TagFilter::And(tags) => {
                    sql.push_str(" AND b.tags @> $2 ");
                    Some(tags)
                }
                TagFilter::Or(tags) => {
                    sql.push_str(" AND b.tags && $2 ");
                    Some(tags)
                }
                TagFilter::Untagged => {
                    sql.push_str(" AND cardinality(b.tags) = 0 ");
                    None
                }
                TagFilter::Any => None,
            };
            (sql, None, tags)
        }
        (Some(tag_filter), Some(query)) => {
            sql.push_str(" AND b.search_tokens @@ websearch_to_tsquery('english', $2) ");
            let tags: Option<Vec<String>> = match tag_filter {
                TagFilter::And(tags) => {
                    sql.push_str(" AND b.tags @> $3 ");
                    Some(tags)
                }
                TagFilter::Or(tags) => {
                    sql.push_str(" AND b.tags && $3 ");
                    Some(tags)
                }
                TagFilter::Untagged => {
                    sql.push_str(" AND cardinality(b.tags) = 0 ");
                    None
                }
                TagFilter::Any => None,
            };
            (sql, Some(query), tags)
        }
    }
}
