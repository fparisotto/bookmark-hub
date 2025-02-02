use anyhow::Result;
use chrono::Utc;
use futures::future::join_all;
use lol_html::{element, rewrite_str, RewriteStrSettings};
use reqwest::Client;
use shared::Bookmark;
use std::collections::HashMap;
use tracing::instrument;
use url::Url;

use crate::readability;

#[derive(Debug)]
#[allow(dead_code)] // FIXME
pub struct Image {
    pub id: String,
    pub original_url: String,
    pub original_src: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug)]
struct ImageFound {
    id: String,
    src: String,
    url: Url,
}

#[instrument(skip(http))]
pub async fn process_url(
    http: &Client,
    readability_url: Url,
    original_url_str: &str,
) -> Result<(Bookmark, Vec<Image>, String, String)> {
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

    let (html_content, images) =
        rewrite_images(&bookmark_id, &readability_response.content, images_index).await?;

    let bookmark = Bookmark {
        bookmark_id,
        url: original_url.to_string(),
        domain: super::domain_from_url(&original_url)?,
        title: readability_response.title,
        created_at: Utc::now(),
        links: None,
        user_id: None,
        tags: None,
        user_created_at: Utc::now(),
        user_updated_at: None,
    };

    Ok((
        bookmark,
        images,
        html_content,
        readability_response.text_content,
    ))
}

#[instrument(skip(content, images_found))]
async fn rewrite_images(
    bookmark_id: &str,
    content: &str,
    images_found: HashMap<String, Image>,
) -> Result<(String, Vec<Image>)> {
    let element_content_handlers = vec![element!("img[src]", |el| {
        let img_src = el.get_attribute("src").expect("img[src] was required");
        let new_src = match &images_found.get(&img_src) {
            Some(image_found) => {
                let src = format!(
                    "/static/{bookmark_id}/{image_found_id}",
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

#[instrument(skip(http))]
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

#[instrument(skip(client))]
async fn fetch_html_content(client: &Client, url: &Url) -> Result<String> {
    Ok(client.get(url.to_string()).send().await?.text().await?)
}
