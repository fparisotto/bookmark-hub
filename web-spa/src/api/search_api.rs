use chrono::{DateTime, Utc};
use gloo_net::http::Request;
use gloo_net::Error;
use serde::{Deserialize, Serialize};
use strum_macros::EnumString;
use uuid::Uuid;

use crate::api::PUBLIC_API_ENDPOINT;

use super::tags_api::Tag;

#[derive(Debug, PartialEq, Default, Clone, EnumString, Serialize, Deserialize)]
pub enum SearchType {
    #[default]
    #[strum(ascii_case_insensitive)]
    Query,

    #[strum(ascii_case_insensitive)]
    Phrase,
}

#[derive(Debug, PartialEq, Default, Clone, EnumString, Serialize, Deserialize)]
pub enum TagFilterType {
    #[default]
    #[strum(ascii_case_insensitive)]
    Or,

    #[strum(ascii_case_insensitive)]
    And,
}

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub struct SearchResultItem {
    pub bookmark_id: String,
    pub url: String,
    pub domain: String,
    pub title: String,
    pub search_match: Option<String>,
    pub links: Option<Vec<String>>,
    pub created_at: DateTime<Utc>,
    pub user_id: Option<Uuid>,
    pub tags: Option<Vec<String>>,
    pub user_created_at: Option<DateTime<Utc>>,
    pub user_updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum TagFilter {
    And(Vec<String>),
    Or(Vec<String>),
    Any,
    Untagged,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchRequest {
    pub query: Option<String>,
    pub tags_filter: Option<TagFilter>,
    pub limit: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResponse {
    pub bookmarks: Vec<SearchResultItem>,
    pub tags: Vec<Tag>,
}

pub async fn search(token: &String, request: SearchRequest) -> Result<SearchResponse, Error> {
    let endpoint = format!("{PUBLIC_API_ENDPOINT}/api/v1/search");
    let request_body = serde_json::to_string(&request).expect("Serialize should not fail");
    let response = Request::post(&endpoint)
        .header("Authorization", &format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .body(request_body)
        .send()
        .await?
        .json::<SearchResponse>()
        .await?;
    log::info!(
        "Api search, request={}",
        serde_json::to_string(&request).unwrap()
    );

    Ok(response)
}
