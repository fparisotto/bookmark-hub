use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolResult, ContentBlock, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo,
};
use rmcp::service::RequestContext;
use rmcp::{tool, tool_handler, tool_router, ErrorData as McpError, RoleServer, ServerHandler};
use shared::{
    BookmarkTaskSearchRequest, BookmarkTaskStatus, Bookmarks, RagHistoryRequest, RagQueryRequest,
    SearchRequest, TagCount, TagFilter, TagOperation, TagsWithCounters,
};
use tracing::{error, info, warn};
use url::Url;

use super::tools::{
    AppendTagsParams, CreateBookmarkParams, DeleteBookmarkParams, GetBookmarkParams,
    GetBookmarksByTagParams, ListTasksParams, RagHistoryParams, RagQueryParams,
    SearchBookmarksParams, SetTagsParams,
};
use crate::db::{bookmark, bookmark_task, rag as rag_db, search as search_db};
use crate::endpoints::Claim;
use crate::error::Error as AppError;
use crate::rag::RagEngine;
use crate::AppContext;

#[derive(Clone)]
pub struct BookmarkMcpServer {
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
}

fn map_err(error: AppError) -> McpError {
    match error {
        AppError::NotFound => McpError::resource_not_found("not_found", None),
        AppError::Unauthorized | AppError::InvalidToken | AppError::WrongCredentials => {
            McpError::invalid_params("unauthorized", None)
        }
        AppError::BadRequest { .. } | AppError::UnprocessableEntity { .. } => {
            McpError::invalid_params(error.to_string(), None)
        }
        ref other => McpError::internal_error(other.to_string(), None),
    }
}

fn parse_task_status(s: &str) -> Result<BookmarkTaskStatus, McpError> {
    match s.to_lowercase().as_str() {
        "pending" => Ok(BookmarkTaskStatus::Pending),
        "done" => Ok(BookmarkTaskStatus::Done),
        "fail" => Ok(BookmarkTaskStatus::Fail),
        other => Err(McpError::invalid_params(
            format!("unknown status '{other}'"),
            None,
        )),
    }
}

/// Pull the validated Claim and AppContext from the request extensions
/// carried by `RequestContext`.
fn auth_ctx(ctx: &RequestContext<RoleServer>) -> Result<(&Claim, &AppContext), McpError> {
    let parts = ctx
        .extensions
        .get::<axum::http::request::Parts>()
        .ok_or_else(|| McpError::internal_error("no http request parts on context", None))?;
    let claim = parts
        .extensions
        .get::<Claim>()
        .ok_or_else(|| McpError::internal_error("no authenticated claim on request", None))?;
    let app_ctx = parts
        .extensions
        .get::<AppContext>()
        .ok_or_else(|| McpError::internal_error("no app context on request", None))?;
    Ok((claim, app_ctx))
}

fn ok_json<T: serde::Serialize>(value: &T) -> Result<CallToolResult, McpError> {
    let body = serde_json::to_string_pretty(value)
        .map_err(|e| McpError::internal_error(format!("serialization failed: {e}"), None))?;
    Ok(CallToolResult::success(vec![ContentBlock::text(body)]))
}

fn ok_text(text: impl Into<String>) -> Result<CallToolResult, McpError> {
    Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
}

#[tool_router]
impl BookmarkMcpServer {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        description = "List all bookmarks for the authenticated user, ordered by creation time."
    )]
    async fn list_bookmarks(
        &self,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let (claim, app_ctx) = auth_ctx(&ctx)?;
        let bookmarks = bookmark::get_by_user(&app_ctx.pool, claim.user_id)
            .await
            .map_err(map_err)?;
        ok_json(&Bookmarks { bookmarks })
    }

    #[tool(description = "Fetch a single bookmark by its identifier.")]
    async fn get_bookmark(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(params): Parameters<GetBookmarkParams>,
    ) -> Result<CallToolResult, McpError> {
        let (claim, app_ctx) = auth_ctx(&ctx)?;
        match bookmark::get_with_user_data(&app_ctx.pool, claim.user_id, &params.bookmark_id).await
        {
            Ok(Some(bookmark)) => ok_json(&bookmark),
            Ok(None) => Err(McpError::resource_not_found("bookmark not found", None)),
            Err(e) => Err(map_err(e)),
        }
    }

    #[tool(
        description = "Queue a new URL for ingestion. A background daemon fetches and indexes the page content, then attaches AI-generated tags and summary."
    )]
    async fn create_bookmark(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(params): Parameters<CreateBookmarkParams>,
    ) -> Result<CallToolResult, McpError> {
        let (claim, app_ctx) = auth_ctx(&ctx)?;
        let url = Url::parse(&params.url)
            .map_err(|e| McpError::invalid_params(format!("invalid url: {e}"), None))?;
        let mut tags = params.tags.unwrap_or_default();
        tags.retain(|t| !t.trim().is_empty());

        let task = bookmark_task::create(&app_ctx.pool, claim.user_id, url.clone(), tags)
            .await
            .map_err(map_err)?;
        if let Err(err) = app_ctx.tx_new_task.send(()) {
            error!(?err, "failed to notify ingestion daemon of new task");
        }
        info!(task_id = %task.task_id, url = %url, "bookmark task created via mcp");
        ok_json(&task)
    }

    #[tool(
        description = "Delete a bookmark by its identifier. Returns success even if the bookmark's static files could not be removed."
    )]
    async fn delete_bookmark(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(params): Parameters<DeleteBookmarkParams>,
    ) -> Result<CallToolResult, McpError> {
        let (claim, app_ctx) = auth_ctx(&ctx)?;
        let deleted = bookmark::delete(&app_ctx.pool, claim.user_id, &params.bookmark_id)
            .await
            .map_err(map_err)?;
        if !deleted {
            return Err(McpError::resource_not_found("bookmark not found", None));
        }
        let static_dir = app_ctx
            .config
            .data_dir
            .join(claim.user_id.to_string())
            .join(&params.bookmark_id);
        if static_dir.exists() {
            if let Err(err) = tokio::fs::remove_dir_all(&static_dir).await {
                error!(
                    bookmark_id = %params.bookmark_id,
                    path = ?static_dir,
                    error = %err,
                    "failed to remove static files for deleted bookmark"
                );
            }
        }
        ok_text("deleted")
    }

    #[tool(
        description = "List all tags used by the authenticated user, with the number of bookmarks using each tag."
    )]
    async fn list_tags(&self, ctx: RequestContext<RoleServer>) -> Result<CallToolResult, McpError> {
        let (claim, app_ctx) = auth_ctx(&ctx)?;
        let raw = bookmark::get_tag_count_by_user(&app_ctx.pool, claim.user_id)
            .await
            .map_err(map_err)?;
        let tags = raw
            .into_iter()
            .map(|(tag, count)| TagCount { tag, count })
            .collect::<Vec<_>>();
        ok_json(&TagsWithCounters { tags })
    }

    #[tool(description = "List all bookmarks carrying a given tag (case-insensitive).")]
    async fn get_bookmarks_by_tag(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(params): Parameters<GetBookmarksByTagParams>,
    ) -> Result<CallToolResult, McpError> {
        let (claim, app_ctx) = auth_ctx(&ctx)?;
        let tag = params.tag.to_lowercase();
        let bookmarks = bookmark::get_by_tag(&app_ctx.pool, claim.user_id, &tag)
            .await
            .map_err(map_err)?;
        ok_json(&Bookmarks { bookmarks })
    }

    #[tool(description = "Replace the tags on a bookmark with the provided list.")]
    async fn set_tags(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(params): Parameters<SetTagsParams>,
    ) -> Result<CallToolResult, McpError> {
        let (claim, app_ctx) = auth_ctx(&ctx)?;
        let updated = bookmark::update_tags(
            &app_ctx.pool,
            claim.user_id,
            &params.bookmark_id,
            &TagOperation::Set(params.tags),
        )
        .await
        .map_err(map_err)?;
        ok_json(&updated)
    }

    #[tool(
        description = "Append tags to a bookmark. Existing tags are preserved; duplicates are removed."
    )]
    async fn append_tags(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(params): Parameters<AppendTagsParams>,
    ) -> Result<CallToolResult, McpError> {
        let (claim, app_ctx) = auth_ctx(&ctx)?;
        let updated = bookmark::update_tags(
            &app_ctx.pool,
            claim.user_id,
            &params.bookmark_id,
            &TagOperation::Append(params.tags),
        )
        .await
        .map_err(map_err)?;
        ok_json(&updated)
    }

    #[tool(
        description = "Full-text + tag-filter search over the authenticated user's bookmarks. Returns matched bookmarks, an aggregate of tags, and the total count."
    )]
    async fn search_bookmarks(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(params): Parameters<SearchBookmarksParams>,
    ) -> Result<CallToolResult, McpError> {
        let (claim, app_ctx) = auth_ctx(&ctx)?;
        let tags_filter = match (params.tags.as_ref(), params.tags_filter_type.as_deref()) {
            (None, _) | (_, Some("any")) => Some(TagFilter::Any),
            (Some(_), Some("untagged")) => Some(TagFilter::Untagged),
            (Some(tags), Some("and")) => Some(TagFilter::And(tags.clone())),
            (Some(tags), Some("or") | None) => Some(TagFilter::Or(tags.clone())),
            (Some(_), Some(other)) => {
                return Err(McpError::invalid_params(
                    format!("unknown tags_filter_type '{other}'"),
                    None,
                ));
            }
        };
        let request = SearchRequest {
            query: params.query,
            tags_filter,
            limit: params.limit,
            offset: params.offset,
        };
        let response = search_db::search(&app_ctx.pool, claim.user_id, &request)
            .await
            .map_err(map_err)?;
        ok_json(&response)
    }

    #[tool(
        description = "Paginated search of bookmark ingestion tasks (pending/done/fail) for the authenticated user."
    )]
    async fn list_tasks(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(params): Parameters<ListTasksParams>,
    ) -> Result<CallToolResult, McpError> {
        let (claim, app_ctx) = auth_ctx(&ctx)?;
        let status = match params.status.as_deref() {
            Some(s) => Some(parse_task_status(s)?),
            None => None,
        };
        let last_task_id = match params.last_task_id.as_deref() {
            Some(s) => Some(s.parse::<uuid::Uuid>().map_err(|e| {
                McpError::invalid_params(format!("invalid last_task_id: {e}"), None)
            })?),
            None => None,
        };
        let request = BookmarkTaskSearchRequest {
            url: params.url,
            status,
            tags: params.tags,
            from_created_at: None,
            to_created_at: None,
            page_size: params.page_size,
            last_task_id,
        };
        let response = bookmark_task::search(&app_ctx.pool, claim.user_id, &request)
            .await
            .map_err(map_err)?;
        ok_json(&response)
    }

    #[tool(
        description = "Ask a question and answer it using the content of your saved bookmarks (RAG). Requires an LLM provider to be configured on the server."
    )]
    async fn rag_query(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(params): Parameters<RagQueryParams>,
    ) -> Result<CallToolResult, McpError> {
        let (claim, app_ctx) = auth_ctx(&ctx)?;
        let llm_client = match &app_ctx.llm_client {
            Some(client) => client.clone(),
            None => {
                warn!("rag_query called via mcp but llm is not configured");
                return Err(McpError::invalid_params(
                    "AI features are not available: LLM provider is not configured.",
                    None,
                ));
            }
        };
        let request = RagQueryRequest {
            question: params.question,
            max_chunks: params.max_chunks,
            similarity_threshold: params.similarity_threshold,
            max_context_tokens: params.max_context_tokens,
            hybrid_search: None,
        };
        let engine = RagEngine::new(app_ctx.pool.clone(), llm_client);
        match engine.process_query(claim.user_id, &request).await {
            Ok(response) => ok_json(&response),
            Err(err) => {
                warn!(?err, "mcp rag_query failed");
                Err(McpError::internal_error(
                    format!("failed to process rag query: {err}"),
                    None,
                ))
            }
        }
    }

    #[tool(
        description = "List past RAG question/answer sessions for the authenticated user, with the source chunks that backed each answer."
    )]
    async fn rag_history(
        &self,
        ctx: RequestContext<RoleServer>,
        Parameters(params): Parameters<RagHistoryParams>,
    ) -> Result<CallToolResult, McpError> {
        let (claim, app_ctx) = auth_ctx(&ctx)?;
        let request = RagHistoryRequest {
            limit: params.limit,
            offset: params.offset,
        };
        let response = rag_db::get_rag_history(&app_ctx.pool, claim.user_id, &request)
            .await
            .map_err(map_err)?;
        ok_json(&response)
    }
}

#[tool_handler]
impl ServerHandler for BookmarkMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .build(),
        )
        .with_server_info(Implementation::from_build_env())
        .with_protocol_version(ProtocolVersion::V_2025_11_25)
        .with_instructions(
            "bookmark-hub MCP server. Tools manage bookmarks and tags, search saved content, and answer questions over indexed bookmarks via RAG."
                .to_string(),
        )
    }
}
