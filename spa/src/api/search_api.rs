use gloo_net::http::Request;
use gloo_net::Error;
use shared::{SearchRequest, SearchResponse};

pub async fn search(token: &String, request: SearchRequest) -> Result<SearchResponse, Error> {
    const ENDPOINT: &str = "/api/v1/search";
    let request_body = serde_json::to_string(&request).expect("Serialize should not fail");
    let response = Request::post(ENDPOINT)
        .header("Authorization", &format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .body(request_body)?
        .send()
        .await?
        .json::<SearchResponse>()
        .await?;
    log::info!(
        "Api search, request={}",
        serde_json::to_string(&request).unwrap()
    );

    Ok(response)
}
