use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderMap, StatusCode};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::Router;
use jsonwebtoken::{decode, DecodingKey, Validation};
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::tower::StreamableHttpService as TowerStreamableHttpService;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use secrecy::ExposeSecret;
use tracing::warn;

use crate::endpoints::Claim;
use crate::AppContext;

mod server;
mod tools;

use server::BookmarkMcpServer;

/// Validate the `Authorization: Bearer <jwt>` header using the same HS256
/// secret the REST API uses, then stash the resulting `Claim` in request
/// extensions so MCP tool handlers can read it.
///
/// `AppContext` is read from request extensions (installed by the outer
/// `Extension(app_state)` layer in `main.rs`).
async fn mcp_bearer_auth(
    headers: HeaderMap,
    request: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let ctx = request
        .extensions()
        .get::<AppContext>()
        .cloned()
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    let bearer = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "));

    let bearer = match bearer {
        Some(b) => b,
        None => return Err(StatusCode::UNAUTHORIZED),
    };

    let decoder = DecodingKey::from_secret(ctx.config.hmac_key.expose_secret().as_bytes());
    match decode::<Claim>(bearer, &decoder, &Validation::default()) {
        Ok(data) => {
            let mut request = request;
            request.extensions_mut().insert(data.claims);
            Ok(next.run(request).await)
        }
        Err(err) => {
            warn!(?err, "mcp bearer token rejected");
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}

/// Build the `/mcp` router. Mount the rmcp `StreamableHttpService` behind
/// the bearer-token middleware. Stateful session management is disabled
/// (`NeverSessionManager`); each request is processed independently.
///
/// `AppContext` is expected to already be installed as an `Extension` on
/// the outer application (see `main.rs`); the auth middleware reads it from
/// request extensions and the MCP tools read it from the per-request
/// `Parts.extensions`.
pub fn router() -> Router {
    let service: TowerStreamableHttpService<BookmarkMcpServer, LocalSessionManager> =
        StreamableHttpService::new(
            || Ok(BookmarkMcpServer::new()),
            LocalSessionManager::default().into(),
            StreamableHttpServerConfig::default(),
        );

    Router::new()
        .nest_service("/mcp", service)
        .layer(middleware::from_fn(mcp_bearer_auth))
}
