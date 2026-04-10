use axum::routing::post;
use axum::{Extension, Json, Router};
use axum_macros::debug_handler;
use shared::{
    HybridSearchConfig, RagHistoryRequest, RagHistoryResponse, RagQueryRequest, RagQueryResponse,
};
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

fn validate_weighted_hybrid_config(config: &HybridSearchConfig) -> Result<()> {
    let vector_weight = config.vector_weight.unwrap_or(0.5);
    let fts_weight = config.fts_weight.unwrap_or(0.5);
    let mut errors = Vec::new();

    for (field, value) in [("vector_weight", vector_weight), ("fts_weight", fts_weight)] {
        if !value.is_finite() {
            errors.push((field, "must be a finite number"));
        } else if value < 0.0 {
            errors.push((field, "must be greater than or equal to 0"));
        }
    }

    if errors.is_empty() && vector_weight + fts_weight <= 0.0 {
        errors.push((
            "hybrid_search",
            "vector_weight and fts_weight cannot both be zero",
        ));
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(Error::unprocessable_entity(errors))
    }
}

fn validate_rag_query_request(request: &RagQueryRequest) -> Result<()> {
    let Some(config) = request.hybrid_search.as_ref() else {
        return Ok(());
    };
    if !config.enabled || config.use_rrf.unwrap_or(true) {
        return Ok(());
    }
    validate_weighted_hybrid_config(config)
}

#[debug_handler]
async fn rag_query(
    claims: Claim,
    Extension(app_context): Extension<AppContext>,
    Json(request): Json<RagQueryRequest>,
) -> Result<Json<RagQueryResponse>> {
    validate_rag_query_request(&request)?;
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

#[cfg(test)]
mod tests {
    use shared::{HybridSearchConfig, RagQueryRequest};

    use super::validate_rag_query_request;

    #[test]
    fn weighted_hybrid_rejects_zero_total_weight() {
        let request = RagQueryRequest {
            question: "test".into(),
            max_chunks: None,
            similarity_threshold: None,
            max_context_tokens: None,
            hybrid_search: Some(HybridSearchConfig {
                enabled: true,
                use_rrf: Some(false),
                rrf_k: None,
                vector_weight: Some(0.0),
                fts_weight: Some(0.0),
            }),
        };

        assert!(validate_rag_query_request(&request).is_err());
    }

    #[test]
    fn weighted_hybrid_rejects_negative_weight() {
        let request = RagQueryRequest {
            question: "test".into(),
            max_chunks: None,
            similarity_threshold: None,
            max_context_tokens: None,
            hybrid_search: Some(HybridSearchConfig {
                enabled: true,
                use_rrf: Some(false),
                rrf_k: None,
                vector_weight: Some(-0.1),
                fts_weight: Some(1.0),
            }),
        };

        assert!(validate_rag_query_request(&request).is_err());
    }

    #[test]
    fn weighted_hybrid_accepts_positive_weights() {
        let request = RagQueryRequest {
            question: "test".into(),
            max_chunks: None,
            similarity_threshold: None,
            max_context_tokens: None,
            hybrid_search: Some(HybridSearchConfig {
                enabled: true,
                use_rrf: Some(false),
                rrf_k: None,
                vector_weight: Some(0.2),
                fts_weight: Some(0.8),
            }),
        };

        assert!(validate_rag_query_request(&request).is_ok());
    }
}
