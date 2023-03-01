pub mod auth_api;
pub mod bookmarks_api;
pub mod search_api;
pub mod tags_api;

pub const PUBLIC_API_ENDPOINT: &'static str = std::env!("PUBLIC_API_ENDPOINT");
