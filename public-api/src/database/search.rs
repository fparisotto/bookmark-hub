use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres, QueryBuilder};
use tokio::try_join;
use tracing::instrument;
use uuid::Uuid;

use crate::error::{Error, Result};

#[derive(Debug, Deserialize)]
pub enum TagFilterType {
    And,
    Or,
}

#[derive(Debug, Deserialize)]
pub struct SearchRequest {
    query: Option<String>,
    phrase: Option<String>,
    tags: Option<Vec<String>>,
    tags_filter_type: Option<TagFilterType>,
    limit: Option<i32>,
}

impl SearchRequest {
    pub fn validate(&self) -> Result<()> {
        let mut errors: Vec<(&'static str, &'static str)> = Vec::new();
        if self.query.is_some() && self.phrase.is_some() {
            errors.push(("query", "query and phrase are mutually exclusive"));
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(Error::unprocessable_entity(errors))
        }
    }
}

#[derive(Debug, sqlx::FromRow, Serialize)]
struct TagCount {
    tag: String,
    count: i64,
}

#[derive(Serialize)]
pub struct SearchResponse {
    bookmarks: Vec<SearchResultItem>,
    tags: Vec<TagCount>,
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

pub struct SearchService;

impl SearchService {
    fn search_query_builder<'a>(
        user_id: &'a Uuid,
        request: &'a SearchRequest,
        tag_aggregation: bool,
    ) -> QueryBuilder<'a, Postgres> {
        let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new("");
        if tag_aggregation {
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
        } else {
            query_builder.push("select ");
            match (&request.query, &request.phrase) {
                (Some(query), None) => {
                    query_builder
                        .push(" ts_headline('english', b.text_content, to_tsquery('english', ")
                        .push_bind(query)
                        .push(" ), 'StartSel=<mark>, StopSel=</mark>') as search_match, ");
                }
                (None, Some(phrase)) => {
                    query_builder
                        .push(
                            " ts_headline('english', b.text_content, phraseto_tsquery('english', ",
                        )
                        .push_bind(phrase)
                        .push(" ), 'StartSel=<mark>, StopSel=</mark>') as search_match, ");
                }
                _ => {
                    query_builder.push(" null as search_match, ");
                }
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
        }
        query_builder
            .push(" where bu.user_id = ")
            .push_bind(user_id);

        match (&request.tags_filter_type, &request.tags) {
            (Some(TagFilterType::And), Some(tags)) if !tags.is_empty() => {
                query_builder.push(" and bu.tags @> ").push_bind(tags);
            }
            (Some(TagFilterType::Or), Some(tags)) if !tags.is_empty() => {
                query_builder.push(" and bu.tags && ").push_bind(tags);
            }
            _ => (),
        }

        match (&request.query, &request.phrase) {
            (Some(query), None) => {
                query_builder
                    .push(" and b.search_tokens @@ to_tsquery('english', ")
                    .push_bind(query)
                    .push(" ) order by ts_rank(b.search_tokens, to_tsquery('english', ")
                    .push_bind(query)
                    .push(" )) ");
            }
            (None, Some(phrase)) => {
                query_builder
                    .push(" and b.search_tokens @@ phraseto_tsquery('english', ")
                    .push_bind(phrase)
                    .push(" ) order by ts_rank(b.search_tokens, phraseto_tsquery('english', ")
                    .push_bind(phrase)
                    .push(" )) ");
            }
            _ => (),
        }

        if tag_aggregation {
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
        } else {
            let limit_value = request.limit.unwrap_or(20);
            query_builder.push(" limit ").push_bind(limit_value);
        }
        query_builder
    }

    #[instrument]
    async fn run_search(
        db: &Pool<Postgres>,
        user_id: &Uuid,
        request: &SearchRequest,
    ) -> Result<Vec<SearchResultItem>> {
        let mut search_query: QueryBuilder<Postgres> =
            SearchService::search_query_builder(user_id, request, false);
        tracing::debug!("Search query {}", &search_query.sql());
        let bookmarks: Vec<SearchResultItem> = search_query.build_query_as().fetch_all(db).await?;
        Ok(bookmarks)
    }

    #[instrument]
    async fn run_aggregation(
        db: &Pool<Postgres>,
        user_id: &Uuid,
        request: &SearchRequest,
    ) -> Result<Vec<TagCount>> {
        let mut aggregation_query: QueryBuilder<Postgres> =
            SearchService::search_query_builder(user_id, request, true);
        tracing::debug!("Aggregation query {}", &aggregation_query.sql());
        let tags: Vec<TagCount> = aggregation_query.build_query_as().fetch_all(db).await?;
        Ok(tags)
    }

    #[instrument]
    pub async fn search(
        db: &Pool<Postgres>,
        user_id: &Uuid,
        request: SearchRequest,
    ) -> Result<SearchResponse> {
        // FIXME this mimics a meillisearch query, replace me in future
        let f_search = SearchService::run_search(db, user_id, &request);
        let f_aggregation = SearchService::run_aggregation(db, user_id, &request);
        let (bookmarks, tags) = try_join!(f_search, f_aggregation)?;
        Ok(SearchResponse { bookmarks, tags })
    }
}
