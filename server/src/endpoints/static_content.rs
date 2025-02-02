use axum::Router;
use tower_http::services::fs::ServeDir;

use crate::Config;

pub fn routes(config: &Config) -> Router {
    Router::new().nest_service("/static", ServeDir::new(&config.data_dir))
}
