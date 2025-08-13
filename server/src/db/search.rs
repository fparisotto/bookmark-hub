use chrono::{DateTime, Utc};
use deadpool_postgres::GenericClient;
use futures::TryFutureExt;
use postgres_from_row::FromRow;
use postgres_types::ToSql;
use serde::{Deserialize, Serialize};
use shared::{Bookmark, SearchRequest, SearchResponse, SearchResultItem, TagCount, TagFilter};
use tokio::try_join;
use tracing::{debug, warn};
use uuid::Uuid;

use crate::error::{Error, Result};

use super::PgPool;

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
    let mut client = pool.get().await?;
    let transaction = client.transaction().await?;

    let f_search = run_search(&transaction, user_id, request).map_err(|e| {
        warn!("Search query fail");
        e
    });
    let f_aggregation = run_aggregation(&transaction, user_id, request).map_err(|e| {
        warn!("Aggregation query fail");
        e
    });
    let f_total = run_total(&transaction, user_id, request).map_err(|e| {
        warn!("Total query fail");
        e
    });
    let (items, tags, total) = try_join!(f_search, f_aggregation, f_total)?;

    transaction.commit().await?;

    Ok(SearchResponse { items, tags, total })
}

async fn run_total(
    client: &impl GenericClient,
    user_id: Uuid,
    request: &SearchRequest,
) -> Result<u64> {
    let mut filters: Vec<String> = vec![];
    let mut params: Vec<&(dyn ToSql + Sync)> = vec![];

    params.push(&user_id);
    filters.push(format!("b.user_id = ${}", params.len()));

    if let Some(query) = &request.query {
        params.push(query);
        filters.push(format!(
            "b.search_tokens @@ websearch_to_tsquery('english', ${})",
            params.len()
        ));
    }

    if let Some(tag_filter) = &request.tags_filter {
        match tag_filter {
            TagFilter::And(tags) => {
                params.push(tags);
                filters.push(format!("b.tags @> ${}", params.len()));
            }
            TagFilter::Or(tags) => {
                params.push(tags);
                filters.push(format!("b.tags && ${}", params.len()));
            }
            TagFilter::Untagged => {
                filters.push("cardinality(b.tags) = 0".to_string());
            }
            TagFilter::Any => { /* No filter */ }
        }
    }

    let filter_clause = format!("WHERE {}", filters.join(" AND "));
    let sql = format!("SELECT COUNT(1) FROM bookmark b {filter_clause}");

    debug!(?sql, "Total query");
    let row = client.query_one(&sql, &params).await?;
    let total: i64 = row.try_get(0)?;
    Ok(total as u64)
}

async fn run_aggregation(
    client: &impl GenericClient,
    user_id: Uuid,
    request: &SearchRequest,
) -> Result<Vec<TagCount>> {
    let mut filters: Vec<String> = vec![];
    let mut params: Vec<&(dyn ToSql + Sync)> = vec![];

    params.push(&user_id);
    filters.push(format!("b.user_id = ${}", params.len()));

    if let Some(query) = &request.query {
        params.push(query);
        filters.push(format!(
            "b.search_tokens @@ websearch_to_tsquery('english', ${})",
            params.len()
        ));
    }

    if let Some(tag_filter) = &request.tags_filter {
        match tag_filter {
            TagFilter::And(tags) => {
                params.push(tags);
                filters.push(format!("b.tags @> ${}", params.len()));
            }
            TagFilter::Or(tags) => {
                params.push(tags);
                filters.push(format!("b.tags && ${}", params.len()));
            }
            TagFilter::Untagged => {
                filters.push("cardinality(b.tags) = 0".to_string());
            }
            TagFilter::Any => { /* No filter */ }
        }
    }

    let filter_clause = format!("WHERE {}", filters.join(" AND "));
    let sql = format!(
        "WITH tags AS (SELECT unnest(b.tags) AS tag FROM bookmark b {filter_clause}) \
         SELECT tag, count(1) AS count FROM tags t GROUP BY tag",
    );

    debug!(?sql, "Aggregation query");

    client
        .query(&sql, &params)
        .await?
        .iter()
        .map(|row| {
            RowTagCount::try_from_row(row)
                .map(TagCount::from)
                .map_err(Error::from)
        })
        .collect::<Result<Vec<_>>>()
}

async fn run_search(
    client: &impl GenericClient,
    user_id: Uuid,
    request: &SearchRequest,
) -> Result<Vec<SearchResultItem>> {
    let mut params: Vec<&(dyn ToSql + Sync)> = vec![];
    let mut filters: Vec<String> = vec![];
    let mut order_by_clause = "ORDER BY b.created_at DESC".to_string();

    let select_clause;
    let query_param_idx: Option<usize>;
    // Necessary because we need a stable memory location for the param borrow
    let none_query_param = None::<String>;

    if let Some(query) = &request.query {
        params.push(query);
        let idx = params.len();
        query_param_idx = Some(idx);
        select_clause = format!("ts_headline('english', b.text_content, websearch_to_tsquery('english', ${idx}), 'StartSel=<mark>, StopSel=</mark>') AS search_match, b.*");
        order_by_clause = format!(
            "ORDER BY ts_rank(b.search_tokens, websearch_to_tsquery('english', ${idx})) DESC",
        );
    } else {
        params.push(&none_query_param);
        query_param_idx = None;
        select_clause = format!("${}::text AS search_match, b.*", params.len());
    }

    params.push(&user_id);
    filters.push(format!("b.user_id = ${}", params.len()));

    if let Some(idx) = query_param_idx {
        filters.push(format!(
            "b.search_tokens @@ websearch_to_tsquery('english', ${idx})",
        ));
    }

    if let Some(tag_filter) = &request.tags_filter {
        match tag_filter {
            TagFilter::And(tags) => {
                params.push(tags);
                filters.push(format!("b.tags @> ${}", params.len()));
            }
            TagFilter::Or(tags) => {
                params.push(tags);
                filters.push(format!("b.tags && ${}", params.len()));
            }
            TagFilter::Untagged => {
                filters.push("cardinality(b.tags) = 0".to_string());
            }
            TagFilter::Any => {}
        }
    }

    let filter_clause = format!("WHERE {}", filters.join(" AND "));
    let limit_clause = format!("LIMIT {}", request.limit.unwrap_or(20));
    let offset_clause = if let Some(offset) = request.offset {
        format!("OFFSET {}", offset)
    } else {
        String::new()
    };
    let sql = format!(
        "SELECT {select_clause} FROM bookmark b {filter_clause} {order_by_clause} {limit_clause} {offset_clause}"
    );

    debug!(?sql, "Search query");

    client
        .query(&sql, &params)
        .await?
        .iter()
        .map(|row| {
            RowSearchResultItem::try_from_row(row)
                .map(SearchResultItem::from)
                .map_err(Error::from)
        })
        .collect::<Result<Vec<_>>>()
}
