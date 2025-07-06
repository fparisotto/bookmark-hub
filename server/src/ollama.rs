use anyhow::bail;
use ollama_rs::{
    generation::{
        completion::request::GenerationRequest,
        embeddings::request::{EmbeddingsInput, GenerateEmbeddingsRequest},
        parameters::{FormatType, JsonSchema, JsonStructure},
    },
    Ollama,
};
use serde::Deserialize;

use url::Url;

#[derive(JsonSchema, Deserialize)]
struct TagsModelResponse {
    tags: Vec<String>,
}

#[derive(JsonSchema, Deserialize)]
struct SummaryModelResponse {
    summary: String,
}

const SYSTEM_PROMPT: &str = r#"You are an expert researcher. Follow these instructions when responding:
  - The user is a highly experienced analyst, no need to simplify it, be as detailed as possible and make sure your response is correct.
  - Be highly organized.
  - Treat me as an expert in all subject matter.
  - Mistakes erode my trust, so be accurate and thorough.
  - Be succinct and cohesive.
"#;

pub async fn tags(ollama_url: &Url, ollama_model: &str, text: &str) -> anyhow::Result<Vec<String>> {
    const PROMPT_PREFIX: &str = r#"The given the following text is a slice of a bigger article.
    I'm asking you to indentify a set of tags that better describe this slice of text.
    Each tag should be a preferably word or short description of the subject related on this text.
    Most of the text will be related to programming and technology, so be focus on that.
    Avoid tags that is too broad or generic, try to focus on what will make this piece distinct.
    Is ok to not provide any tag if you think there's nothing relevant to point out.
    Response should be in JSON format. Here's text: "#;

    let ollama = Ollama::from_url(ollama_url.to_owned());
    let format = FormatType::StructuredJson(Box::new(JsonStructure::new::<TagsModelResponse>()));
    let request =
        GenerationRequest::new(ollama_model.to_owned(), format!("{PROMPT_PREFIX}\n{text}"))
            .format(format)
            .system(SYSTEM_PROMPT);
    let response = ollama.generate(request).await?;
    if let Ok(parsed) = serde_json::from_str::<TagsModelResponse>(&response.response) {
        Ok(parsed.tags)
    } else {
        bail!(
            "Ollama failed to extract tags from chunk, text: {}, response: {}",
            text,
            response.response
        )
    }
}

pub async fn consolidate_tags(
    ollama_url: &Url,
    ollama_model: &str,
    tags: Vec<String>,
) -> anyhow::Result<Vec<String>> {
    const PROMPT_PREFIX: &str = r#"I'll give you a list of tags, they come from slices of an article.
    Most of the tags are related to programming and technology.
    Each tag is word or short description of the subject related on this text.
    Most of the tags looks like duplicated, redundant or ambiguous.
    Try to produce a new list of tags that is more clear and succinct.
    Less is better, give me maximum of 15 tags.
    Response should be in JSON format. Here're the tags, comma separated: "#;

    let text = tags.join(", ");
    let ollama = Ollama::from_url(ollama_url.to_owned());
    let format = FormatType::StructuredJson(Box::new(JsonStructure::new::<TagsModelResponse>()));
    let request =
        GenerationRequest::new(ollama_model.to_owned(), format!("{PROMPT_PREFIX}\n{text}"))
            .format(format)
            .system(SYSTEM_PROMPT);
    let response = ollama.generate(request).await?;
    if let Ok(parsed) = serde_json::from_str::<TagsModelResponse>(&response.response) {
        Ok(parsed.tags)
    } else {
        bail!(
            "Ollama failed to extract tags from chunk, text: {}, response: {}",
            text,
            response.response
        )
    }
}

pub async fn summary(ollama_url: &Url, ollama_model: &str, text: &str) -> anyhow::Result<String> {
    const PROMPT_PREFIX: &str = r#"The given the following text is a slice of a bigger article.
    I'm asking you to produce a sort summary of it taht better describe this slice of text.
    Most of the text will be related to programming and technology, so be focus on that.
    Avoid topics that is too broad or generic, try to focus on what will make this piece distinct.
    Is ok to not provide any summary if you think there's nothing relevant to point out.
    Try to be very succinct, one sentence would be the ideal.
    Response should be in JSON format. Here's text: "#;

    let ollama = Ollama::from_url(ollama_url.to_owned());

    let format = FormatType::StructuredJson(Box::new(JsonStructure::new::<SummaryModelResponse>()));
    let request =
        GenerationRequest::new(ollama_model.to_owned(), format!("{PROMPT_PREFIX}\n{text}"))
            .format(format);
    let response = ollama.generate(request).await?;
    if let Ok(parsed) = serde_json::from_str::<SummaryModelResponse>(&response.response) {
        Ok(parsed.summary)
    } else {
        bail!(
            "Ollama failed to extract summary from chunk, text: {}, response: {}",
            text,
            response.response
        )
    }
}

pub async fn consolidate_summary(
    ollama_url: &Url,
    ollama_model: &str,
    summaries: &[String],
) -> anyhow::Result<String> {
    const PROMPT_PREFIX: &str = r#"I'll give you a list of summaries, they come from slices of an article.
    Most of the summary are related to programming and technology.
    Most of the summaries looks like duplicated, redundant or ambiguous.
    Try to produce a new consolidated summary of then all, that is more clear and succinct.
    Less is better, give me maximum of 3 sentences.
    Response should be in JSON format. Here're the summaries, separated by new lines: "#;

    let text = summaries.join("\n");
    let ollama = Ollama::from_url(ollama_url.to_owned());
    let format = FormatType::StructuredJson(Box::new(JsonStructure::new::<SummaryModelResponse>()));
    let request =
        GenerationRequest::new(ollama_model.to_owned(), format!("{PROMPT_PREFIX}\n{text}"))
            .format(format);
    let response = ollama.generate(request).await?;
    if let Ok(parsed) = serde_json::from_str::<SummaryModelResponse>(&response.response) {
        Ok(parsed.summary)
    } else {
        bail!(
            "Ollama failed to extract summary from chunk, text: {}, response: {}",
            text,
            response.response
        )
    }
}

#[allow(dead_code)]
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
        Some(embeddings) => Ok(embeddings.to_owned()),
        None => {
            bail!("No embeddings returned from ollama")
        }
    }
}
