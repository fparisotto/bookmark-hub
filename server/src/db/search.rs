use chrono::{DateTime, Utc};
use deadpool_postgres::GenericClient;
use futures::TryFutureExt;
use postgres_from_row::FromRow;
use serde::{Deserialize, Serialize};
use tokio::try_join;
use tracing::{debug, instrument, warn};
use uuid::Uuid;

use crate::error::{Error, Result};

use super::{PgConnection, PgPool};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
struct TagCount {
    tag: String,
    count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    bookmarks: Vec<SearchResultItem>,
    tags: Vec<TagCount>,
    total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TagFilter {
    And(Vec<String>),
    Or(Vec<String>),
    Any,
    Untagged,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchRequest {
    query: Option<String>,
    tags_filter: Option<TagFilter>,
    limit: Option<i32>,
}

#[instrument(skip(pool))]
pub async fn search(
    pool: &PgPool,
    user_id: Uuid,
    request: &SearchRequest,
) -> Result<SearchResponse> {
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
    let (bookmarks, tags, total) = try_join!(f_search, f_aggregation, f_total)?;
    Ok(SearchResponse {
        bookmarks,
        tags,
        total,
    })
}

#[instrument(skip(client))]
async fn run_total(client: &PgConnection, user_id: Uuid, request: &SearchRequest) -> Result<u64> {
    let sql: String = "SELECT COUNT(1) FROM bookmark_user bu
        INNER JOIN bookmark b USING (bookmark_id) WHERE bu.user_id = $1 "
        .to_owned();
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

#[instrument(skip(client))]
async fn run_aggregation(
    client: &PgConnection,
    user_id: Uuid,
    request: &SearchRequest,
) -> Result<Vec<TagCount>> {
    let sql = "WITH tags AS (
        SELECT unnest(bu.tags) AS tag FROM bookmark_user bu
        INNER JOIN bookmark b USING(bookmark_id)
        WHERE bu.user_id = $1 "
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
                .map(|row| TagCount::try_from_row(row).map_err(Error::from))
                .collect::<Result<Vec<_>>>()?;
            Ok(result)
        }
        (None, Some(tags)) => {
            let result = client
                .query(&sql, &[&user_id, &tags])
                .await?
                .iter()
                .map(|row| TagCount::try_from_row(row).map_err(Error::from))
                .collect::<Result<Vec<_>>>()?;
            Ok(result)
        }
        (Some(query), None) => {
            let result = client
                .query(&sql, &[&user_id, &query])
                .await?
                .iter()
                .map(|row| TagCount::try_from_row(row).map_err(Error::from))
                .collect::<Result<Vec<_>>>()?;
            Ok(result)
        }
        (Some(query), Some(tags)) => {
            let result = client
                .query(&sql, &[&user_id, &query, &tags])
                .await?
                .iter()
                .map(|row| TagCount::try_from_row(row).map_err(Error::from))
                .collect::<Result<Vec<_>>>()?;
            Ok(result)
        }
    }
}

#[instrument(skip(client))]
async fn run_search(
    client: &PgConnection,
    user_id: Uuid,
    request: &SearchRequest,
) -> Result<Vec<SearchResultItem>> {
    let mut sql = "SELECT ".to_owned();
    let query_select = match request.query.clone() {
        Some(query) => {
            if query.trim().starts_with('"') && query.trim().ends_with('"') {
                sql.push_str(" ts_headline('english', b.text_content, phraseto_tsquery('english', $1), 'StartSel=<mark>, StopSel=</mark>') AS search_match, ");
                Some(query)
            } else {
                sql.push_str(" ts_headline('english', b.text_content, to_tsquery('english', $1), 'StartSel=<mark>, StopSel=</mark>') AS search_match, ");
                let query = if query.contains('&') {
                    query.to_owned()
                } else {
                    query.split(' ').collect::<Vec<_>>().join(" | ")
                };
                Some(query)
            }
        }
        None => {
            sql.push_str(" $1 AS search_match, ");
            None
        }
    };

    sql.push_str(" b.*, bu.user_id, bu.tags, bu.created_at AS user_created_at, bu.updated_at AS user_updated_at");
    sql.push_str(" FROM bookmark_user bu INNER JOIN bookmark b USING(bookmark_id) ");
    sql.push_str(" WHERE bu.user_id = $2 ");

    let tags = match request.tags_filter.clone().unwrap_or(TagFilter::Any) {
        TagFilter::And(tags) => {
            sql.push_str(" AND bu.tags @> $3 ");
            Some(tags)
        }
        TagFilter::Or(tags) => {
            sql.push_str(" AND bu.tags && $3 ");
            Some(tags)
        }
        TagFilter::Untagged => {
            sql.push_str(" AND cardinality(bu.tags) = 0 AND $3 IS NULL ");
            None
        }
        TagFilter::Any => {
            sql.push_str(" AND CAST($3 AS TEXT[]) IS NULL ");
            None
        }
    };

    let query_filter: Option<String> = match request.query.clone() {
        Some(query) => {
            if query.trim().starts_with('"') && query.trim().ends_with('"') {
                sql.push_str(" AND b.search_tokens @@ phraseto_tsquery('english', $4) ");
                sql.push_str(
                    " ORDER BY ts_rank(b.search_tokens, phraseto_tsquery('english', $4)) ",
                );
                Some(query)
            } else {
                let query = if query.contains('&') {
                    query
                } else {
                    query.split(' ').collect::<Vec<_>>().join(" | ")
                };
                sql.push_str(" AND b.search_tokens @@ to_tsquery('english', $4) ");
                sql.push_str(" ORDER BY ts_rank(b.search_tokens, to_tsquery('english', $4)) ");
                Some(query)
            }
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
        .map(|row| SearchResultItem::try_from_row(row).map_err(Error::from))
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
            let query = if query.trim().starts_with('"') && query.trim().ends_with('"') {
                sql.push_str(" AND b.search_tokens @@ phraseto_tsquery('english', $2) ");
                query
            } else {
                sql.push_str(" AND b.search_tokens @@ to_tsquery('english', $2) ");
                if query.contains('&') {
                    query
                } else {
                    query.split(' ').collect::<Vec<_>>().join(" | ")
                }
            };
            (sql, Some(query), None)
        }
        (Some(tag_filter), None) => {
            let tags = match tag_filter {
                TagFilter::And(tags) => {
                    sql.push_str(" AND bu.tags @> $2 ");
                    Some(tags)
                }
                TagFilter::Or(tags) => {
                    sql.push_str(" AND bu.tags && $2 ");
                    Some(tags)
                }
                TagFilter::Untagged => {
                    sql.push_str(" AND cardinality(bu.tags) = 0 ");
                    None
                }
                TagFilter::Any => None,
            };
            (sql, None, tags)
        }
        (Some(tag_filter), Some(query)) => {
            let query = if query.trim().starts_with('"') && query.trim().ends_with('"') {
                sql.push_str(" AND b.search_tokens @@ phraseto_tsquery('english', $2) ");
                query
            } else {
                sql.push_str(" AND b.search_tokens @@ to_tsquery('english', $2) ");
                if query.contains('&') {
                    query
                } else {
                    query.split(' ').collect::<Vec<_>>().join(" | ")
                }
            };
            let tags: Option<Vec<String>> = match tag_filter {
                TagFilter::And(tags) => {
                    sql.push_str(" AND bu.tags @> $3 ");
                    Some(tags)
                }
                TagFilter::Or(tags) => {
                    sql.push_str(" AND bu.tags && $3 ");
                    Some(tags)
                }
                TagFilter::Untagged => {
                    sql.push_str(" AND cardinality(bu.tags) = 0 ");
                    None
                }
                TagFilter::Any => None,
            };
            (sql, Some(query), tags)
        }
    }
}
