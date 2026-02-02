use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use anyhow::Context;
use clap::{Args, Parser, Subcommand};
use reqwest::Client;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use shared::{NewBookmarkRequest, NewBookmarkResponse, SignInResponse};
use tracing::level_filters::LevelFilter;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, EnvFilter};
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

    #[command(about = "Import bookmarks from Firefox HTML export")]
    ImportFirefox(ImportFirefoxArgs),
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

#[derive(Debug, Clone, Args)]
pub struct ImportFirefoxArgs {
    #[arg(long, help = "Path to Firefox bookmark HTML export file")]
    pub file: PathBuf,

    #[arg(long, help = "Folder name to import bookmarks from")]
    pub folder: String,

    #[arg(long, help = "Parse and list URLs without importing or saving state")]
    pub dry_run: bool,
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
        InnerCommand::ImportFirefox(args) => handle_import_firefox(args).await?,
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

async fn handle_import_firefox(args: ImportFirefoxArgs) -> anyhow::Result<()> {
    // Parse the HTML file
    let html_content = fs::read_to_string(&args.file)
        .with_context(|| format!("Failed to read file: {}", args.file.display()))?;
    let urls = parse_firefox_bookmarks(&html_content, &args.folder)?;

    if urls.is_empty() {
        tracing::warn!("No bookmarks found in folder '{}'", args.folder);
        return Ok(());
    }

    let total_found = urls.len();
    tracing::info!(
        "Found {} bookmarks in folder '{}'",
        total_found,
        args.folder
    );

    // Load previously imported URLs
    let imported = load_imported_urls()?;
    let new_urls: Vec<_> = urls.into_iter().filter(|u| !imported.contains(u)).collect();

    let total_skipped = total_found - new_urls.len();
    if total_skipped > 0 {
        tracing::info!(
            "Skipping {} already-imported URLs, importing {} new URLs",
            total_skipped,
            new_urls.len()
        );
    }

    if new_urls.is_empty() {
        tracing::info!("All bookmarks already imported");
        return Ok(());
    }

    // Dry run: just list the URLs without importing
    if args.dry_run {
        tracing::info!("Dry run mode - listing {} URLs to import:", new_urls.len());
        for url_str in &new_urls {
            tracing::info!("  {}", url_str);
        }
        return Ok(());
    }

    let (token, base_url) = load_token_and_url()?;
    let client = Client::new();

    let mut success_count = 0;
    let mut failure_count = 0;

    for url_str in new_urls {
        let url: Url = match url_str.parse() {
            Ok(u) => u,
            Err(e) => {
                tracing::error!("Invalid URL '{}': {}", url_str, e);
                failure_count += 1;
                continue;
            }
        };

        let request = NewBookmarkRequest {
            url: url.clone().into(),
            tags: Default::default(),
        };

        match add_bookmark(&client, &base_url, &token, request).await {
            Ok(response) => {
                tracing::info!(?response, %url, "Added bookmark");
                save_imported_url(&url_str)?;
                success_count += 1;
            }
            Err(error) => {
                tracing::error!(?error, %url, "Failed to add bookmark");
                failure_count += 1;
            }
        }
    }

    tracing::info!(
        "Import complete: {} succeeded, {} failed, {} skipped",
        success_count,
        failure_count,
        total_skipped
    );

    Ok(())
}

fn parse_firefox_bookmarks(html: &str, folder_name: &str) -> anyhow::Result<Vec<String>> {
    let document = Html::parse_document(html);
    let h3_selector = Selector::parse("h3").expect("valid selector");
    let a_selector = Selector::parse("a").expect("valid selector");
    let dt_selector = Selector::parse("dt").expect("valid selector");
    let dl_selector = Selector::parse("dl").expect("valid selector");

    // Find the H3 element with matching folder name
    let mut target_dl = None;
    for h3 in document.select(&h3_selector) {
        let text = h3.text().collect::<String>();
        if text.trim() == folder_name {
            // Firefox HTML structure after parsing: <DT><H3>name</H3><DL>...</DL></DT>
            // The DL is a sibling of the H3 within the same DT
            if let Some(parent_dt) = h3.parent() {
                if let Some(parent_ref) = scraper::ElementRef::wrap(parent_dt) {
                    // Look for DL inside the parent DT
                    if let Some(dl) = parent_ref.select(&dl_selector).next() {
                        target_dl = Some(dl);
                        break;
                    }
                }
            }
        }
    }

    let dl = target_dl.with_context(|| format!("Folder '{}' not found in HTML", folder_name))?;

    // Extract only direct A links (skip nested subfolders)
    let mut urls = Vec::new();
    let dl_id = dl.id();
    for dt in dl.select(&dt_selector) {
        // Check if this DT is a direct child of the target DL
        let is_direct_child = dt.parent().map(|p| p.id() == dl_id).unwrap_or(false);
        if !is_direct_child {
            continue;
        }

        // Skip if this DT contains an H3 (it's a subfolder)
        if dt.select(&h3_selector).next().is_some() {
            continue;
        }

        // Extract the A href
        if let Some(a) = dt.select(&a_selector).next() {
            if let Some(href) = a.value().attr("href") {
                // Skip javascript: and place: URLs
                if !href.starts_with("javascript:") && !href.starts_with("place:") {
                    urls.push(href.to_string());
                }
            }
        }
    }

    Ok(urls)
}

fn get_imported_urls_path() -> anyhow::Result<PathBuf> {
    let config_path = home::home_dir()
        .context("Missing home dir")?
        .join(".config/bookmark-hub");
    fs::create_dir_all(&config_path)?;
    Ok(config_path.join("imported.txt"))
}

fn load_imported_urls() -> anyhow::Result<HashSet<String>> {
    let path = get_imported_urls_path()?;
    if !path.exists() {
        return Ok(HashSet::new());
    }
    let content = fs::read_to_string(&path)?;
    Ok(content.lines().map(|s| s.to_string()).collect())
}

fn save_imported_url(url: &str) -> anyhow::Result<()> {
    let path = get_imported_urls_path()?;
    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
    writeln!(file, "{}", url)?;
    Ok(())
}
