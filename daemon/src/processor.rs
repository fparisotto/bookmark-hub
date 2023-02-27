use anyhow::{anyhow, Result};
use futures::future::join_all;
use lol_html::{element, rewrite_str, RewriteStrSettings};
use murmur3::murmur3_x64_128;
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::io::Cursor;
use url::Url;

#[derive(Debug)]
pub struct Bookmark {
    pub id: String,
    pub original_url: String,
    pub domain: String,
    pub title: String,
    pub html: String,
    pub text: String,
    pub images: Vec<Image>,
    // TODO links: Vec<String>
}

#[derive(Debug)]
pub struct Image {
    pub id: String,
    pub original_url: String,
    pub original_src: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
}

struct ReadabilityResponse {
    title: String,
    html: String,
    text: String,
}

struct ImageFound {
    id: String,
    src: String,
    url: Url,
}

pub async fn process_url(
    http: &Client,
    readability_endpoint: &str,
    original_url_str: &str,
    static_image_endpoint: &str,
    static_prefix: &str,
) -> Result<Bookmark> {
    let original_url = Url::parse(original_url_str)?;
    let original_url = clean_url(&original_url)?;
    let bookmark_id: String = make_id(&original_url)?;
    let raw_html = fetch_html_content(http, &original_url).await?;
    let readability_response = readability_process(http, readability_endpoint, raw_html).await?;

    let images_found = find_images(&original_url, &readability_response.html)?;

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

    let (new_content, images) = rewrite_images(
        static_image_endpoint,
        static_prefix,
        &bookmark_id,
        &readability_response.html,
        images_index,
    )
    .await?;

    let bookmark = Bookmark {
        id: bookmark_id,
        original_url: original_url.to_string(),
        domain: domain_from_url(&original_url)?,
        title: readability_response.title.clone(),
        html: new_content,
        text: readability_response.text.clone(),
        images,
    };

    Ok(bookmark)
}

pub fn clean_url(url: &Url) -> Result<Url> {
    if let Some(host) = url.host_str() {
        let path = &url.path();
        let clean_url = format!("{scheme}://{host}{path}", scheme = &url.scheme());
        tracing::info!("Clean url={clean_url}");
        let clean = Url::parse(&clean_url)?;
        return Ok(clean);
    }
    Err(anyhow!(format!("Invalid url={url}")))
}

fn make_id(url: &Url) -> Result<String> {
    if let Some(host) = url.host_str() {
        let path = url.path();
        let source = format!("{host}.{path}");
        let mut source = Cursor::new(source.as_str());
        let hash = murmur3_x64_128(&mut source, 0)?;
        let id = base64_url::encode(&hash.to_be_bytes());
        tracing::info!(id = &id, url = format!("{url}"), "Making url id");
        return Ok(id);
    }
    Err(anyhow!(format!("Invalid url={url}")))
}

fn domain_from_url(url: &Url) -> Result<String> {
    let domain_or_host = url
        .domain()
        .or_else(|| url.host_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!(format!("Domain not found for url={url}")))?;
    Ok(domain_or_host)
}

async fn rewrite_images(
    static_image_endpoint: &str,
    static_prefix: &str,
    bookmark_id: &str,
    content: &str,
    images_found: HashMap<String, Image>,
) -> Result<(String, Vec<Image>)> {
    let element_content_handlers = vec![element!("img[src]", |el| {
        let img_src = el.get_attribute("src").expect("img[src] was required");
        let new_src = match &images_found.get(&img_src) {
            Some(image_found) => {
                let src = format!(
                    "{static_image_endpoint}/{static_prefix}/{bookmark_id}/{image_found_id}",
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
                let image_id: String = make_id(&parsed)?;
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

#[derive(Deserialize)]
struct ReadabilityPayload {
    title: String,
    content: String,
    #[serde(rename(deserialize = "textContent"))]
    text_content: String,
}

async fn readability_process(
    client: &Client,
    readability_endpoint: &str,
    raw_content: String,
) -> Result<ReadabilityResponse> {
    let response = client
        .post(readability_endpoint)
        .body(raw_content)
        .send()
        .await?;
    let payload = response.json::<ReadabilityPayload>().await?;
    Ok(ReadabilityResponse {
        title: payload.title,
        html: payload.content,
        text: payload.text_content,
    })
}
