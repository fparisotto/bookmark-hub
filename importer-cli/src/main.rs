use anyhow::Result;
use reqwest::{self, StatusCode};
use serde::{Deserialize, Serialize};
use std::env;
use std::{
    fs::File,
    io::{BufReader, Read},
};
use url::Url;
use uuid::Uuid;

#[derive(Serialize)]
struct Credentials<'a> {
    email: &'a str,
    password: &'a str,
}

#[derive(Deserialize)]
struct SignInResponse {
    user_id: Uuid,
    access_token: String,
    email: String,
    token_type: String,
}

#[derive(Serialize)]
struct NewBookmark {
    url: Url,
    tags: Vec<String>,
}

#[derive(Deserialize)]
pub struct NewBookmarkResponse {
    pub task_id: Uuid,
    pub user_id: Uuid,
    pub url: String,
    pub status: String,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    if !(args.len() >= 3 && args.len() <= 4) {
        eprintln!("Args: {:?}", args);
        return Err(anyhow::anyhow!(
            "Wrong arguments, import usage: cli <username> <password> <file> [endpoint]"
        ));
    }

    let email = args.get(0).expect("email");
    let password = args.get(1).expect("password");
    let file_path = args.get(2).expect("file_path");
    let endpoint = args
        .get(3)
        .unwrap_or(&"http://localhost:3000".to_owned())
        .to_owned();

    println!("Using endpoint: {endpoint}");
    let credentials = Credentials { email, password };

    let http = reqwest::Client::new();

    println!("Doing sign-in to get new token...");
    let response = http
        .post(format!("{endpoint}/api/v1/auth/sign-in"))
        .json(&credentials)
        .send()
        .await?;
    if response.status() != StatusCode::OK {
        let message = response.text().await?;
        return Err(anyhow::anyhow!(
            "Sign-in attempt fail, wrong credentials, message={message}"
        ));
    }
    let sign_in = response.json::<SignInResponse>().await?;
    println!(
        "Sign-in succeeded, user_id: {user_id}, email: {email}, token_type: {token_type}",
        user_id = sign_in.user_id,
        email = sign_in.email,
        token_type = sign_in.token_type,
    );

    let file = File::open(file_path)?;
    let mut reader = BufReader::new(file);
    let mut content = String::new();
    reader.read_to_string(&mut content)?;
    for line in content.lines() {
        // TODO make a file format with tags
        let url = Url::parse(line)?;

        let body = NewBookmark {
            url: url.clone(),
            tags: vec![],
        };
        let response = http
            .post(format!("{endpoint}/api/v1/bookmarks"))
            .header(
                "Authorization",
                format!("Bearer {token}", token = &sign_in.access_token),
            )
            .json(&body)
            .send()
            .await?;

        if response.status() != StatusCode::CREATED {
            eprintln!(
                "Fail to process, moving forward. url: {url}, status={status}, response={body}",
                status = response.status(),
                body = response.text().await?,
            );
            continue;
        }

        let response = response.json::<NewBookmarkResponse>().await?;
        println!(
            "Task ID: {task}, url: {url}",
            task = response.task_id,
            url = response.url
        );
    }

    println!("DONE!");

    Ok(())
}
