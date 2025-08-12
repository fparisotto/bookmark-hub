use anyhow::Context;
use clap::{Args, Parser, Subcommand};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use shared::{NewBookmarkRequest, NewBookmarkResponse, SignInResponse};
use std::fs;
use std::path::PathBuf;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{EnvFilter, fmt};
use url::Url;

#[derive(Debug, Clone, Parser)]
#[command(version)]
pub struct CliArgs {
    #[clap(subcommand)]
    pub command: InnerCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum InnerCommand {
    #[command(about = "Login with email and password")]
    Login(LoginArgs),

    #[command(about = "Add a bookmark")]
    Add(AddArgs),

    #[command(about = "Add multiple bookmarks from a file")]
    AddBatch(AddBatchArgs),
}

#[derive(Debug, Clone, Args)]
pub struct LoginArgs {
    #[arg(long, help = "API base URL")]
    pub url: Url,

    #[arg(long, help = "Username")]
    pub username: String,

    #[arg(long, help = "Login password")]
    pub password: String,
}

#[derive(Debug, Clone, Args)]
pub struct AddArgs {
    #[arg(long, help = "Url to bookmark")]
    pub url: Url,
}

#[derive(Debug, Clone, Args)]
pub struct AddBatchArgs {
    #[arg(long, help = "File with one URL per line")]
    pub file: PathBuf,
}

#[derive(Debug, Serialize, Deserialize)]
struct StoredAuth {
    pub base_url: Url,
    pub response: SignInResponse,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .init();
    let args = CliArgs::parse();
    match args.command {
        InnerCommand::Login(args) => handle_login(args).await?,
        InnerCommand::Add(args) => handle_add(args).await?,
        InnerCommand::AddBatch(args) => handle_add_batch(args).await?,
    }
    Ok(())
}

async fn handle_login(args: LoginArgs) -> anyhow::Result<()> {
    let home_dir = home::home_dir().context("Expect to have a home dir for user")?;
    let config_path = home_dir.join(".config/bookmark-hub");
    fs::create_dir_all(&config_path)?;
    let config_file = config_path.join("auth.json");
    let auth_response = login(&args.url, &args.username, &args.password)
        .await
        .context("Failed to login")?;
    let stored = StoredAuth {
        base_url: args.url,
        response: auth_response,
    };
    let json = serde_json::to_string_pretty(&stored)?;
    fs::write(&config_file, json)?;
    tracing::info!("Token saved at {}", config_file.display());
    Ok(())
}

async fn handle_add(args: AddArgs) -> anyhow::Result<()> {
    let (token, base_url) = load_token_and_url()?;
    let client = Client::new();
    let request = NewBookmarkRequest {
        url: args.url.into(),
        tags: Default::default(),
    };
    let response = add_bookmark(&client, &base_url, &token, request)
        .await
        .context("Failed to add bookmark")?;
    tracing::info!(?response, "Add bookmakr");
    Ok(())
}

async fn handle_add_batch(args: AddBatchArgs) -> anyhow::Result<()> {
    let (token, base_url) = load_token_and_url()?;
    let client = Client::new();
    let content = fs::read_to_string(&args.file)?;
    for (idx, line) in content.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let url: Url = line
            .parse()
            .with_context(|| format!("Invalid URL on line {}", idx + 1))?;
        let request = NewBookmarkRequest {
            url: url.clone().into(),
            tags: Default::default(),
        };
        match add_bookmark(&client, &base_url, &token, request).await {
            Ok(response) => tracing::info!(?response, %url, "Add bookmark"),
            Err(error) => tracing::error!(?error, %url, "Failed to add bookmark"),
        }
    }
    Ok(())
}

async fn login(base_url: &Url, email: &str, password: &str) -> anyhow::Result<SignInResponse> {
    let endpoint = base_url.join("/api/v1/auth/sign-in")?;
    let client = Client::new();
    let payload = serde_json::json!({
        "username": email,
        "password": password,
    });
    let response = client
        .post(endpoint)
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await?
        .error_for_status()?
        .json::<SignInResponse>()
        .await?;
    Ok(response)
}

async fn add_bookmark(
    client: &Client,
    base_url: &Url,
    token: &str,
    request: NewBookmarkRequest,
) -> anyhow::Result<NewBookmarkResponse> {
    let endpoint = base_url.join("/api/v1/bookmarks")?;
    let response = client
        .post(endpoint)
        .bearer_auth(token)
        .json(&request)
        .send()
        .await?
        .error_for_status()?
        .json::<NewBookmarkResponse>()
        .await?;
    Ok(response)
}

fn load_token_and_url() -> anyhow::Result<(String, Url)> {
    let config_path = home::home_dir()
        .context("Missing home dir")?
        .join(".config/bookmark-hub/auth.json");
    let content = fs::read_to_string(&config_path).context("Expected auth file")?;
    let stored: StoredAuth = serde_json::from_str(&content)?;
    Ok((stored.response.access_token, stored.base_url))
}
