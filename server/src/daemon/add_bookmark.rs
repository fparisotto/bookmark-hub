use anyhow::{Context, Result};
use chrono::Utc;
use futures::future::join_all;
use lol_html::{element, rewrite_str, RewriteStrSettings};
use reqwest::Client;
use reqwest::Client as HttpClient;
use shared::{Bookmark, BookmarkTask, BookmarkTaskStatus};
use std::collections::HashMap;
use url::Url;
use uuid::Uuid;

use crate::db::{self, PgPool};
use crate::readability;
use crate::Config;

use super::DAEMON_IDLE_SLEEP;

const TASK_MAX_RETRIES: i16 = 5;

#[derive(Debug, Clone)]
#[allow(dead_code)] // FIXME: use or remove fields
struct Image {
    id: String,
    original_url: String,
    original_src: String,
    content_type: String,
    bytes: Vec<u8>,
}

#[derive(Debug)]
struct ImageFound {
    id: String,
    src: String,
    url: Url,
}

#[derive(Debug, Clone)]
struct ProcessorOutput {
    bookmark_id: String,
    url: String,
    domain: String,
    title: String,
    text_content: String,
    images: Vec<Image>,
    html: String,
}

pub fn should_retry(task: &BookmarkTask) -> bool {
    task.retries.unwrap_or(0) < TASK_MAX_RETRIES
}

pub async fn run(
    pool: &PgPool,
    config: &Config,
    mut new_task_rx: tokio::sync::watch::Receiver<()>,
    new_bookmark_tx: tokio::sync::watch::Sender<()>,
) -> Result<()> {
    let http: HttpClient = HttpClient::new();
    let mut interval = tokio::time::interval(DAEMON_IDLE_SLEEP);
    loop {
        tokio::select! {
            _ = new_task_rx.changed() => {
                tracing::info!("Notification receive, executing...");
                match execute_step(pool, &http, config).await {
                    Ok(_) => {
                        if let Err(error) = new_bookmark_tx.send(()) {
                            tracing::error!(?error, "Fail to send signal on new bookmark");
                        }
                    },
                    Err(error) => {
                        tracing::error!(?error, "Fail to process tasks");
                    },
                }
            }
            _ = interval.tick() => {
                tracing::info!("{DAEMON_IDLE_SLEEP:?} passed, executing...");
                match execute_step(pool, &http, config).await {
                    Ok(_) => {
                        if let Err(error) = new_bookmark_tx.send(()) {
                            tracing::error!(?error, "Fail to send signal on new bookmark");
                        }
                    },
                    Err(error) => {
                        tracing::error!(?error, "Fail to process tasks");
                    },
                }
            }
        }
    }
}

async fn execute_step(pool: &PgPool, http: &HttpClient, config: &Config) -> Result<()> {
    let tasks: Vec<BookmarkTask> = db::bookmark_task::peek(pool, Utc::now()).await?;
    if tasks.is_empty() {
        tracing::info!("No new task");
        return Ok(());
    }
    tracing::info!("New tasks found: {}", tasks.len());
    for task in tasks {
        tracing::info!(?task, "Executing task");
        match handle_task(pool, http, config, &task).await {
            Ok(_) => {
                db::bookmark_task::update(pool, task.clone(), BookmarkTaskStatus::Done, None, None)
                    .await?;
                tracing::info!(task_uuid = format!("{}", task.task_id), "Task executed")
            }
            Err(error) => {
                if should_retry(&task) {
                    let retry_value: i16 = task.retries.unwrap_or(0) + 1;
                    db::bookmark_task::update(
                        pool,
                        task.clone(),
                        BookmarkTaskStatus::Pending,
                        Some(retry_value),
                        None,
                    )
                    .await?;
                    tracing::warn!(?task, ?error, "Task failed, retying",)
                } else {
                    db::bookmark_task::update(
                        pool,
                        task.clone(),
                        BookmarkTaskStatus::Fail,
                        None,
                        Some(format!("{error}")),
                    )
                    .await?;
                    tracing::error!(?task, ?error, "Task failed");
                }
            }
        }
    }
    Ok(())
}

async fn handle_task(
    pool: &PgPool,
    http: &HttpClient,
    config: &Config,
    task: &BookmarkTask,
) -> Result<()> {
    if db::bookmark::get_by_url_and_user_id(pool, &task.url, task.user_id)
        .await?
        .is_some()
    {
        tracing::info!(?task, "Duplicated bookmark");
        return Ok(());
    }

    tracing::info!("Processing new bookmark for url={}", &task.url);
    let output = process_url(
        http,
        config.readability_url.clone(),
        &task.user_id,
        &task.url,
    )
    .await
    .with_context(|| format!("process_url: {}", &task.url))?;

    let bookmark = Bookmark {
        bookmark_id: output.bookmark_id,
        user_id: task.user_id,
        url: output.url,
        domain: output.domain,
        title: output.title,
        tags: task.tags.to_owned(),
        summary: task.summary.to_owned(),
        created_at: Utc::now(),
        updated_at: None,
    };

    let bookmark_saved = db::bookmark::save(pool, &bookmark, &output.text_content)
        .await
        .with_context(|| {
            format!(
                "save_bookmark_into_database: bookmark_id={}",
                &bookmark.bookmark_id
            )
        })?;

    save_static_content(
        config,
        &bookmark_saved,
        &output.images,
        &output.html,
        &task.user_id,
    )
    .await
    .with_context(|| {
        format!(
            "save_static_content: bookmark_id={}",
            &bookmark_saved.bookmark_id
        )
    })?;

    tracing::info!(
        url = task.url,
        bookmark_id = format!("{}", &bookmark_saved.bookmark_id),
        "Bookmark created",
    );
    Ok(())
}

async fn save_static_content(
    config: &Config,
    bookmark: &Bookmark,
    images: &[Image],
    content: &str,
    user_id: &Uuid,
) -> Result<()> {
    tracing::info!(
        "Saving bookmark, id={}, user_id={}",
        &bookmark.bookmark_id,
        user_id
    );
    let bookmark_dir = config
        .data_dir
        .join(user_id.to_string())
        .join(&bookmark.bookmark_id);

    if !bookmark_dir.exists() {
        tokio::fs::create_dir_all(&bookmark_dir).await?;
    }
    let index = bookmark_dir.join("index.html");
    tokio::fs::write(&index, content).await?;
    for image in images.iter() {
        let image_path = bookmark_dir.join(&image.id);
        if image_path.exists() {
            tracing::info!(?image_path, "Image is already there");
            continue;
        }
        tokio::fs::write(&image_path, &image.bytes).await?;
        tracing::info!(?image_path, "Image file saved");
    }
    Ok(())
}

async fn process_url(
    http: &Client,
    readability_url: Url,
    user_id: &Uuid,
    original_url_str: &str,
) -> Result<ProcessorOutput> {
    let original_url = Url::parse(original_url_str)?;
    let original_url = super::clean_url(original_url)?;
    let bookmark_id: String = super::make_bookmark_id(&original_url)?;
    let raw_html = fetch_html_content(http, &original_url).await?;
    let readability_response = readability::process(http, readability_url, raw_html).await?;

    let images_found = find_images(&original_url, &readability_response.content)?;

    let processed_images = join_all(
        images_found
            .iter()
            .map(|image_found| process_image_found(http, image_found)),
    )
    .await;

    let (images_ok, images_err): (Vec<_>, Vec<_>) =
        processed_images.into_iter().partition(|e| e.is_ok());

    tracing::info!(
        "Images with success: {}, failure: {}",
        images_ok.len(),
        images_err.len()
    );

    images_err.into_iter().for_each(|error| {
        tracing::warn!("Images with error, they will be ignored, error={:?}", error);
    });

    let images_index: HashMap<String, Image> = images_ok
        .into_iter()
        .flat_map(|result| result.ok())
        .map(|image| (image.original_src.clone(), image))
        .collect();

    let (rewrite_html, images) = rewrite_images(
        &bookmark_id,
        user_id,
        &readability_response.content,
        images_index,
    )
    .await?;

    Ok(ProcessorOutput {
        bookmark_id,
        url: original_url.to_string(),
        domain: super::domain_from_url(&original_url)?,
        title: readability_response.title,
        text_content: readability_response.text_content,
        images,
        html: rewrite_html,
    })
}

async fn rewrite_images(
    bookmark_id: &str,
    user_id: &Uuid,
    content: &str,
    images_found: HashMap<String, Image>,
) -> Result<(String, Vec<Image>)> {
    let element_content_handlers = vec![element!("img[src]", |el| {
        let img_src = el.get_attribute("src").expect("img[src] was required");
        let new_src = match &images_found.get(&img_src) {
            Some(image_found) => {
                let src = format!(
                    "/static/{user_id}/{bookmark_id}/{image_found_id}",
                    image_found_id = image_found.id
                );
                tracing::info!("Rewriting image from={img_src}, to={src}");
                src
            }
            None => {
                tracing::warn!(
                    "Something weird happened, processed image not found, img_src={img_src}"
                );
                img_src
            }
        };
        el.set_attribute("src", &new_src)?;
        Ok(())
    })];

    let new_content = rewrite_str(
        content,
        RewriteStrSettings {
            element_content_handlers,
            ..RewriteStrSettings::default()
        },
    )?;

    let images: Vec<Image> = images_found.into_values().collect();
    Ok((new_content, images))
}

async fn process_image_found(http: &Client, image_found: &ImageFound) -> Result<Image> {
    let response = http
        .get(image_found.url.to_string())
        .send()
        .await?
        .error_for_status()?;
    let content_type = response
        .headers()
        .get("Content-Type")
        .map(|v| v.to_str().unwrap_or("application/octet-stream"))
        .unwrap_or("application/octet-stream")
        .to_string();
    let bytes = response.bytes().await?.to_vec();
    Ok(Image {
        id: image_found.id.clone(),
        original_url: image_found.url.to_string(),
        original_src: image_found.src.to_string(),
        content_type,
        bytes,
    })
}

fn find_images(base_url: &Url, content: &str) -> Result<Vec<ImageFound>> {
    let mut images_found: Vec<ImageFound> = Vec::new();

    let element_content_handlers = vec![element!("img[src]", |el| {
        let img_src = el.get_attribute("src").expect("img[src] was required");
        let parsed_img_src = match Url::parse(&img_src) {
            Ok(parsed) => Ok(parsed),
            Err(url::ParseError::RelativeUrlWithoutBase) => {
                tracing::info!("Found relative URL, img_src={img_src}");
                base_url.join(&img_src)
            }
            Err(error) => Err(error),
        };
        match parsed_img_src {
            Ok(parsed) => {
                let image_id: String = super::make_bookmark_id(&parsed)?;
                tracing::info!("Image found, original_url={parsed}");
                images_found.push(ImageFound {
                    id: image_id,
                    url: parsed,
                    src: img_src,
                });
            }
            Err(error) => {
                tracing::warn!(
                    img_src = img_src,
                    "Fail to parse URL from img_src, skipping this image, error={error}"
                );
            }
        };
        Ok(())
    })];

    let _ = rewrite_str(
        content,
        RewriteStrSettings {
            element_content_handlers,
            ..RewriteStrSettings::default()
        },
    )?;

    Ok(images_found)
}

async fn fetch_html_content(client: &Client, url: &Url) -> Result<String> {
    Ok(client.get(url.to_string()).send().await?.text().await?)
}
