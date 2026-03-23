use axum::routing::post;
use axum::{Extension, Json, Router};
use axum_macros::debug_handler;
use shared::{RagHistoryRequest, RagHistoryResponse, RagQueryRequest, RagQueryResponse};
use tracing::{info, warn};

use super::Claim;
use crate::db::rag::get_rag_history;
use crate::error::{Error, Result};
use crate::rag::RagEngine;
use crate::AppContext;

pub fn routes() -> Router {
    Router::new()
        .route("/query", post(rag_query))
        .route("/history", post(rag_history))
}

#[debug_handler]
async fn rag_query(
    claims: Claim,
    Extension(app_context): Extension<AppContext>,
    Json(request): Json<RagQueryRequest>,
) -> Result<Json<RagQueryResponse>> {
    info!(
        user_id = %claims.user_id,
        question = %request.question,
        "RAG query received"
    );

    // Check if LLM is configured
    let llm_client = match &app_context.llm_client {
        Some(client) => client.clone(),
        None => {
            warn!("RAG query attempted but LLM is not configured");
            return Err(Error::bad_request([(
                "llm",
                "AI features are not available. LLM provider is not configured.",
            )]));
        }
    };

    // Create RAG engine
    let rag_engine = RagEngine::new(app_context.pool.clone(), llm_client);

    // Process the query
    match rag_engine.process_query(claims.user_id, &request).await {
        Ok(response) => {
            info!(
                user_id = %claims.user_id,
                session_id = %response.session_id,
                relevant_chunks = response.relevant_chunks.len(),
                "RAG query processed successfully"
            );
            Ok(Json(response))
        }
        Err(error) => {
            warn!(
                user_id = %claims.user_id,
                question = %request.question,
                ?error,
                "RAG query processing failed"
            );
            Err(Error::from(anyhow::anyhow!(
                "Failed to process RAG query: {}",
                error
            )))
        }
    }
}

#[debug_handler]
async fn rag_history(
    claims: Claim,
    Extension(app_context): Extension<AppContext>,
    Json(request): Json<RagHistoryRequest>,
) -> Result<Json<RagHistoryResponse>> {
    info!(
        user_id = %claims.user_id,
        limit = request.limit,
        offset = request.offset,
        "RAG history request received"
    );

    match get_rag_history(&app_context.pool, claims.user_id, &request).await {
        Ok(response) => {
            info!(
                user_id = %claims.user_id,
                sessions_returned = response.sessions.len(),
                total_count = response.total_count,
                "RAG history retrieved successfully"
            );
            Ok(Json(response))
        }
        Err(error) => {
            warn!(
                user_id = %claims.user_id,
                ?error,
                "RAG history retrieval failed"
            );
            Err(error)
        }
    }
}
