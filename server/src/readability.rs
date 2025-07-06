use anyhow::Result;
use dom_smoothie::{Article, Config, Readability};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ReadabilityResponse {
    pub title: String,
    pub content: String,
    #[serde(rename(deserialize = "textContent"))]
    pub text_content: String,
}

pub async fn process(raw_content: String) -> Result<ReadabilityResponse> {
    let mut readability = Readability::new(
        raw_content,
        None,
        Some(Config {
            max_elements_to_parse: usize::MAX,
            ..Default::default()
        }),
    )?;
    let article: Article = readability.parse()?;

    Ok(ReadabilityResponse {
        title: article.title,
        content: article.content.to_string(),
        text_content: article.text_content.to_string(),
    })
}
