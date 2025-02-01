use anyhow::bail;
use ollama_rs::{
    generation::{
        completion::request::GenerationRequest,
        embeddings::request::{EmbeddingsInput, GenerateEmbeddingsRequest},
    },
    Ollama,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::warn;
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TagsModelResponse {
    tags: Vec<String>,
}

pub async fn tags(
    ollama_url: Url,
    ollama_model: String,
    text: String,
) -> anyhow::Result<Vec<String>> {
    const PROMPT_PREFIX: &str = r#"For given the following text set tags for it. Each tag should be a short string.
    Response as this JSON format: '{ "tags": ["some tag", "another tag"] }'.
    Here's text:
    "#;
    let tag_response_schema: Value = json!({
        "required": ["tags"],
        "properties": {
            "tags": {
                "type": "array",
                "items": { "type": "string" }
            }
        }
    });
    let ollama = Ollama::from_url(ollama_url);
    let request =
        GenerationRequest::new(ollama_model.to_owned(), format!("{PROMPT_PREFIX}\n{text}")).format(
            ollama_rs::generation::parameters::FormatType::Json(tag_response_schema),
        );
    let response = ollama.generate(request).await?;
    if let Ok(parsed) = serde_json::from_str::<TagsModelResponse>(&response.response) {
        Ok(parsed.tags)
    } else {
        warn!(%text, response = response.response, "Ollama failed to extract tags from chunk");
        todo!();
    }
}

pub async fn summary(
    ollama_url: Url,
    ollama_model: String,
    text: String,
) -> anyhow::Result<String> {
    const PROMPT: &str = "Summarize the following text, be succinct, maximum 5 sentences:";
    let ollama = Ollama::from_url(ollama_url);
    let request = GenerationRequest::new(ollama_model.clone(), format!("{PROMPT}\n{text}"));
    let response = ollama.generate(request).await?;
    Ok(response.response)
}

pub async fn embeddings(
    ollama_url: Url,
    ollama_model: String,
    text: String,
) -> anyhow::Result<Vec<f32>> {
    let ollama = Ollama::from_url(ollama_url);
    let input = EmbeddingsInput::Single(text);
    let request = GenerateEmbeddingsRequest::new(ollama_model, input);
    let response = ollama.generate_embeddings(request).await?;
    if response.embeddings.len() > 1 {
        bail!("More than one embeddings returned from ollama, not expected")
    }
    match response.embeddings.first() {
        Some(embeddings) => Ok(embeddings.clone()),
        None => {
            bail!("No embeddings returned from ollama")
        }
    }
}
