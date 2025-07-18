use gloo_net::http::Request;
use gloo_net::Error;
use serde_json::json;
use shared::{SignInResponse, UserProfileResponse};

pub async fn login(username: String, password: String) -> Result<SignInResponse, Error> {
    const ENDPOINT: &str = "/api/v1/auth/sign-in";
    let json_value = json!({"username": username, "password": password});
    log::info!("Doing login, endpoint={ENDPOINT}");
    let request_body = serde_json::to_string(&json_value).expect("Serialize should not fail");
    let response = Request::post(ENDPOINT)
        .header("Content-Type", "application/json")
        .body(request_body)?
        .send()
        .await?
        .json::<SignInResponse>()
        .await?;
    log::info!("Api auth login, username={username}");
    Ok(response)
}

pub async fn get_user_profile(token: String) -> Result<UserProfileResponse, Error> {
    const ENDPOINT: &str = "/api/v1/auth/user-profile";
    let response = Request::get(ENDPOINT)
        .header("Authorization", &format!("Bearer {token}"))
        .send()
        .await?
        .json::<UserProfileResponse>()
        .await?;
    log::info!("Api get user profile, token={token}");
    Ok(response)
}
