use chrono::{DateTime, Utc};
use gloo_net::http::Request;
use gloo_net::Error;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::BACKEND_URL;

#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub struct Bookmark {
    pub bookmark_id: String,
    pub url: String,
    pub domain: String,
    pub title: String,
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

#[derive(Debug, Serialize, Deserialize)]
struct Tags {
    tags: Vec<String>,
}

pub async fn add_bookmark(
    token: &String,
    request: NewBookmarkRequest,
) -> Result<NewBookmarkResponse, Error> {
    let endpoint = format!("{BACKEND_URL}/api/v1/bookmarks");
    let request_body = serde_json::to_string(&request).expect("Serialize should not fail");
    let response = Request::post(&endpoint)
        .header("Authorization", &format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .body(request_body)?
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
    let endpoint = format!("{BACKEND_URL}/api/v1/bookmarks/{id}");
    let response = Request::get(&endpoint)
        .header("Authorization", &format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .send()
        .await?;
    log::info!("Api get bookmark by id, id={id}");
    match response.status() {
        200 => {
            let bookmark = response.json::<Bookmark>().await?;
            Ok(Some(bookmark))
        }
        404 => Ok(None),
        _ => {
            let response_body = response.text().await?;
            log::warn!(
                "Api get bookmark by id={id}, error = unexpected response, status={status}, response={response_body}",
                status = response.status(),
            );
            Err(Error::GlooError("unexpected response".to_owned()))
        }
    }
}

pub async fn set_tags(token: &str, id: &str, tags: Vec<String>) -> Result<Bookmark, Error> {
    let endpoint = format!("{BACKEND_URL}/api/v1/bookmarks/{id}/tags");
    let payload = Tags { tags };
    let request_body = serde_json::to_string(&payload).expect("Serialize should not fail");
    let response = Request::post(&endpoint)
        .header("Authorization", &format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .body(request_body)?
        .send()
        .await?
        .json::<Bookmark>()
        .await?;
    log::info!(
        "Api set tags to bookmark={id}, payload={}",
        serde_json::to_string(&payload).unwrap()
    );
    Ok(response)
}

pub async fn get_content(token: &str, id: &str) -> Result<Option<String>, Error> {
    let endpoint = format!("{BACKEND_URL}/static/{id}/index.html");
    let response = Request::get(&endpoint)
        .header("Authorization", &format!("Bearer {token}"))
        .send()
        .await?;
    log::info!("Get static content, id={id}");
    match response.status() {
        200 => {
            let content = response.text().await?;
            Ok(Some(content))
        }
        404 => Ok(None),
        _ => {
            let response_body = response.text().await?;
            log::warn!(
                "Api get bookmark by id={id}, error = unexpected response, status={status}, response={response_body}",
                status = response.status(),
            );
            Err(Error::GlooError("unexpected response".to_owned()))
        }
    }
}
