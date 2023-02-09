use chrono::{DateTime, Utc};
use gloo_net::http::Request;
use gloo_net::Error;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::PUBLIC_API_ENDPOINT;

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub struct Bookmark {
    pub bookmark_id: String,
    pub url: String,
    pub domain: String,
    pub title: String,
    pub html_content: String,
    pub links: Option<Vec<String>>,
    pub created_at: DateTime<Utc>,
    pub user_id: Option<Uuid>,
    pub tags: Option<Vec<String>>,
    pub user_created_at: DateTime<Utc>,
    pub user_updated_at: Option<DateTime<Utc>>,
}

#[derive(PartialEq, Clone, Serialize, Deserialize)]
pub struct NewBookmarkRequest {
    pub url: String,
    pub tags: Vec<String>,
}

#[derive(PartialEq, Clone, Serialize, Deserialize)]
pub struct NewBookmarkResponse {
    pub task_id: Uuid,
    pub url: String,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
}

pub async fn add_bookmark(
    token: &String,
    request: NewBookmarkRequest,
) -> Result<NewBookmarkResponse, Error> {
    let endpoint = format!("{}/api/v1/bookmarks", PUBLIC_API_ENDPOINT);
    let request_body = serde_json::to_string(&request).expect("Serialize should not fail");
    let response = Request::post(&endpoint)
        .header("Authorization", format!("Bearer {}", token).as_str())
        .header("Content-Type", "application/json")
        .body(request_body)
        .send()
        .await?
        .json::<NewBookmarkResponse>()
        .await?;
    log::info!(
        "Api add new bookmark, request={}",
        serde_json::to_string(&request).unwrap()
    );
    Ok(response)
}

pub async fn get_by_id(token: &str, id: &str) -> Result<Option<Bookmark>, Error> {
    let endpoint = format!("{}/api/v1/bookmarks/{}", PUBLIC_API_ENDPOINT, id);
    let response = Request::get(&endpoint)
        .header("Authorization", format!("Bearer {}", token).as_str())
        .header("Content-Type", "application/json")
        .send()
        .await?;
    log::info!( "Api get bookmark by id, id={}", id);
    match response.status() {
        200 => {
            let bookmark = response.json::<Bookmark>().await?;
            Ok(Some(bookmark))
        },
        404 => {
            Ok(None)
        }
        _ => {
            let response_body = response.text().await?;
            log::warn!(
                "Api get bookmark by id={}, error = unexpected response, status={}, response={}",
                id, response.status(), response_body
            );
            Err(Error::GlooError("unexpected response".to_owned()))
        }
    }
}
