use gloo_net::http::Request;
use gloo_net::Error;
use shared::{BookmarkTaskSearchRequest, BookmarkTaskSearchResponse};

pub async fn search_tasks(
    token: &String,
    request: BookmarkTaskSearchRequest,
) -> Result<BookmarkTaskSearchResponse, Error> {
    const ENDPOINT: &str = "/api/v1/tasks";
    let request_body = serde_json::to_string(&request).expect("Serialize should not fail");
    let response = Request::post(ENDPOINT)
        .header("Authorization", &format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .body(request_body)?
        .send()
        .await?
        .json::<BookmarkTaskSearchResponse>()
        .await?;
    log::info!(
        "Api search tasks, request={}",
        serde_json::to_string(&request).unwrap()
    );
    Ok(response)
}
