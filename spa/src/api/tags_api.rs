use gloo_net::http::Request;
use gloo_net::Error;
use shared::TagsResponse;

pub async fn get_all_tags(token: &String) -> Result<TagsResponse, Error> {
    const ENDPOINT: &str = "/api/v1/tags";
    let response = Request::get(ENDPOINT)
        .header("Authorization", &format!("Bearer {token}"))
        .send()
        .await?
        .json::<TagsResponse>()
        .await?;
    log::info!("Api get all tags, token={token}");
    Ok(response)
}
