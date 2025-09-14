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

    // Check if Ollama is configured
    let (ollama_url, text_model) = match (
        &app_context.config.ollama.ollama_url,
        &app_context.config.ollama.ollama_text_model,
    ) {
        (Some(url), Some(model)) => (url.clone(), model.clone()),
        _ => {
            warn!("RAG query attempted but Ollama is not configured");
            return Err(Error::bad_request([(
                "ollama",
                "AI features are not available. Ollama is not configured.",
            )]));
        }
    };

    // Create RAG engine
    let rag_engine = RagEngine::new(app_context.pool.clone(), ollama_url, text_model);

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
