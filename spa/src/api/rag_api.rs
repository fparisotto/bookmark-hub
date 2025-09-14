use gloo_net::http::Request;
use shared::{RagHistoryRequest, RagHistoryResponse, RagQueryRequest, RagQueryResponse};

use crate::user_session::UserSession;

const RAG_API_BASE_URL: &str = "/api/v1/rag";

pub async fn query_rag(
    user_session: &UserSession,
    request: &RagQueryRequest,
) -> Result<RagQueryResponse, gloo_net::Error> {
    let response = Request::post(&format!("{}/query", RAG_API_BASE_URL))
        .header("authorization", &format!("Bearer {}", user_session.token))
        .json(request)?
        .send()
        .await?;

    if response.ok() {
        response.json::<RagQueryResponse>().await
    } else {
        Err(gloo_net::Error::GlooError(format!(
            "RAG query failed with status: {}",
            response.status()
        )))
    }
}

pub async fn get_rag_history(
    user_session: &UserSession,
    request: &RagHistoryRequest,
) -> Result<RagHistoryResponse, gloo_net::Error> {
    let response = Request::post(&format!("{}/history", RAG_API_BASE_URL))
        .header("authorization", &format!("Bearer {}", user_session.token))
        .json(request)?
        .send()
        .await?;

    if response.ok() {
        response.json::<RagHistoryResponse>().await
    } else {
        Err(gloo_net::Error::GlooError(format!(
            "RAG history request failed with status: {}",
            response.status()
        )))
    }
}
