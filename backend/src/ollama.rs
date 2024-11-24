use std::collections::BTreeSet;

use anyhow::bail;
use ollama_rs::{
    generation::{
        completion::request::GenerationRequest,
        embeddings::request::{EmbeddingsInput, GenerateEmbeddingsRequest},
    },
    Ollama,
};
use serde::{Deserialize, Serialize};
use url::Url;

const MAX_WINDOW_CHAR: u32 = 5_000;

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

    const CONSOLITATION_PROMPT: &str = r#"Consolidate all given tags. Remove things you think has the same meaning or are redundant.
    Give me only the final consolidated result. Response as this JSON format: '{ "tags": ["some tag", "another tag"] }'.
    Here's tags:
    "#;

    let ollama = Ollama::from_url(ollama_url);
    let chunks = naive_chunkenizer(&text, MAX_WINDOW_CHAR);
    let mut responses: Vec<TagsModelResponse> = vec![];
    for chunk in chunks {
        let prompt = format!("{PROMPT_PREFIX}\n{chunk}");
        let request = GenerationRequest::new(ollama_model.to_owned(), prompt.to_owned())
            .format(ollama_rs::generation::parameters::FormatType::Json);
        let response = ollama.generate(request).await?;
        let parsed: TagsModelResponse = serde_json::from_str(&response.response)?;
        responses.push(parsed);
    }
    let tags: BTreeSet<_> = responses.into_iter().flat_map(|e| e.tags).collect();
    let prompt = format!("{CONSOLITATION_PROMPT}\n{}", serde_json::to_string(&tags)?);
    let request = GenerationRequest::new(ollama_model, prompt.to_owned())
        .format(ollama_rs::generation::parameters::FormatType::Json);
    let response = ollama.generate(request).await?;
    let parsed: TagsModelResponse = serde_json::from_str(&response.response)?;
    Ok(parsed.tags)
}

pub async fn summary(
    ollama_url: Url,
    ollama_model: String,
    text: String,
) -> anyhow::Result<String> {
    const CONSOLITATION_PROMPT: &str = r#"
    Consolidate all given text, these are a concatenation of summary chunks for the same original text.
    Make it consice and consistent, fixing any issues. Here's the text:
    "#;

    let ollama = Ollama::from_url(ollama_url);
    let chunks = naive_chunkenizer(&text, MAX_WINDOW_CHAR);
    let mut responses: Vec<String> = vec![];
    for chunk in chunks {
        let prompt = format!("Summarize the following text:\n{chunk}");
        let request = GenerationRequest::new(ollama_model.clone(), prompt.to_owned());
        let response = ollama.generate(request).await?;
        responses.push(response.response.clone());
    }
    let prompt = format!("{CONSOLITATION_PROMPT}\n{}", responses.join("\n"));
    let request = GenerationRequest::new(ollama_model, prompt.to_owned());
    let response = ollama.generate(request).await?;
    Ok(response.response)
}

fn naive_chunkenizer(text: &str, max_window_char: u32) -> Vec<String> {
    let lines: Vec<_> = text
        .lines()
        .map(|e| e.trim())
        .filter(|e| !e.is_empty())
        .map(|e| e.to_owned())
        .collect();
    let mut result = vec![];
    let mut buff = String::new();
    for line in lines {
        if buff.len() < max_window_char as usize {
            buff.push_str(&format!("{}\n", line));
        } else {
            result.push(buff.clone());
            buff.clear();
        }
    }
    if !buff.is_empty() {
        result.push(buff.clone());
    }
    result
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
