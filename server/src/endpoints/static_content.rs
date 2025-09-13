use axum::Router;
use tower_http::compression::CompressionLayer;
use tower_http::services::fs::ServeDir;

use crate::Config;

pub fn routes(config: &Config) -> Router {
    Router::new()
        .nest_service(
            "/static",
            ServeDir::new(&config.data_dir).precompressed_gzip(), // Serve .gz files when available
        )
        .layer(CompressionLayer::new()) // Add dynamic compression for
                                        // non-compressed content
}
