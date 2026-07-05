use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetBookmarkParams {
    /// The bookmark identifier (not the URL).
    pub bookmark_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateBookmarkParams {
    /// URL to save. A background daemon fetches and indexes the page content.
    pub url: String,
    /// Optional tags to attach to the bookmark. Empty strings are ignored.
    #[serde(default)]
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteBookmarkParams {
    pub bookmark_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetBookmarksByTagParams {
    /// Tag to filter by (case-insensitive).
    pub tag: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetTagsParams {
    pub bookmark_id: String,
    /// Tags that will replace the bookmark's current tags.
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AppendTagsParams {
    pub bookmark_id: String,
    /// Tags to add to the bookmark. Existing tags are preserved.
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchBookmarksParams {
    /// Full-text search query (websearch syntax). Omit to list bookmarks.
    #[serde(default)]
    pub query: Option<String>,
    /// Tags to filter by. Combined with `tags_filter_type`.
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    /// How to combine `tags`: "and", "or", "any" (ignore tags), or "untagged".
    /// Defaults to "or".
    #[serde(default)]
    pub tags_filter_type: Option<String>,
    /// Max number of results (default 20).
    #[serde(default)]
    pub limit: Option<i32>,
    /// Offset for pagination.
    #[serde(default)]
    pub offset: Option<i32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListTasksParams {
    /// Case-insensitive substring to filter task URLs by.
    #[serde(default)]
    pub url: Option<String>,
    /// Task status to filter by: "pending", "done", or "fail".
    #[serde(default)]
    pub status: Option<String>,
    /// Tags to filter tasks by (all must be present).
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    /// Page size (default 25).
    #[serde(default)]
    pub page_size: Option<u8>,
    /// Task id to start pagination after (ordered by task_id ascending).
    #[serde(default)]
    pub last_task_id: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RagQueryParams {
    /// Natural-language question to answer using the user's bookmark content.
    pub question: String,
    /// Max number of chunks to retrieve (default 6).
    #[serde(default)]
    pub max_chunks: Option<usize>,
    /// Minimum similarity score for a chunk to be considered (default 0.3).
    #[serde(default)]
    pub similarity_threshold: Option<f64>,
    /// Maximum tokens of context to feed the LLM (default 4096).
    #[serde(default)]
    pub max_context_tokens: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RagHistoryParams {
    /// Max number of sessions to return (default 20, max 100).
    #[serde(default)]
    pub limit: Option<usize>,
    /// Offset for pagination.
    #[serde(default)]
    pub offset: Option<usize>,
}
