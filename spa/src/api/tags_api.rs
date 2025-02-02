use gloo_net::http::Request;
use gloo_net::Error;
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub tag: String,
    pub count: u32,
}

#[derive(PartialEq, Serialize, Deserialize)]
pub struct TagsResponse {
    pub tags: Vec<Tag>,
}

pub async fn get_all_tags(token: &String) -> Result<TagsResponse, Error> {
    const ENDPOINT: &str = "/api/v1/tags";
    let response = Request::get(ENDPOINT)
        .header("Authorization", &format!("Bearer {token}"))
        .send()
        .await?
        .json::<TagsResponse>()
        .await?;
    log::info!("Api get all tags, token={}", token);
    Ok(response)
}
