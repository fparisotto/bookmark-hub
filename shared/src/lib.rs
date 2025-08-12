use chrono::{DateTime, Utc};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use strum_macros::{AsRefStr, EnumString};
use url::Url;
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct UserProfile {
    pub user_id: Uuid,
    pub username: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct SignUpRequest {
    pub username: String,
    pub password: SecretString,
    pub password_confirmation: SecretString,
}

#[derive(Debug, Serialize)]
pub struct SignUpResponse {
    pub id: Uuid,
    pub username: String,
}

#[derive(Debug, Deserialize)]
pub struct SignInRequest {
    pub username: String,
    pub password: SecretString,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct SignInResponse {
    pub user_id: Uuid,
    pub username: String,
    pub access_token: String,
    pub token_type: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserProfileResponse {
    pub user_id: Uuid,
    pub username: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bookmark {
    pub bookmark_id: String,
    pub user_id: Uuid,
    pub url: String,
    pub domain: String,
    pub title: String,
    pub tags: Option<Vec<String>>,
    pub summary: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewBookmarkRequest {
    pub url: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewBookmarkResponse {
    pub task_id: Uuid,
    pub url: String,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Default, EnumString, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchResultItem {
    pub bookmark: Bookmark,
    pub search_match: Option<String>,
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
    pub query: Option<String>,
    pub tags_filter: Option<TagFilter>,
    pub limit: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub items: Vec<SearchResultItem>,
    pub tags: Vec<TagCount>,
    pub total: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TagsResponse {
    pub tags: Vec<TagCount>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct TagCount {
    pub tag: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagsWithCounters {
    pub tags: Vec<TagCount>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tags {
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Bookmarks {
    pub bookmarks: Vec<Bookmark>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NewBookmark {
    pub url: Url,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub enum TagOperation {
    Set(Vec<String>),
    Append(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, EnumString, AsRefStr, Default)]
pub enum BookmarkTaskStatus {
    #[default]
    Done,
    Pending,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BookmarkTask {
    pub task_id: Uuid,
    pub user_id: Uuid,
    pub url: String,
    pub status: BookmarkTaskStatus,
    pub tags: Option<Vec<String>>,
    pub summary: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub next_delivery: DateTime<Utc>,
    pub retries: Option<i16>,
    pub fail_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct BookmarkTaskSearchRequest {
    pub url: Option<String>,
    pub status: Option<BookmarkTaskStatus>,
    pub tags: Option<Vec<String>>,
    pub from_created_at: Option<DateTime<Utc>>,
    pub to_created_at: Option<DateTime<Utc>>,
    pub page_size: Option<u8>,
    pub last_task_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BookmarkTaskSearchResponse {
    pub tasks: Vec<BookmarkTask>,
    pub has_more: bool,
    pub total_count: Option<usize>,
}
