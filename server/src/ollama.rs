use std::time::Duration;

use anyhow::bail;
use ollama_rs::generation::completion::request::GenerationRequest;
use ollama_rs::generation::embeddings::request::{EmbeddingsInput, GenerateEmbeddingsRequest};
use ollama_rs::generation::parameters::{FormatType, JsonSchema, JsonStructure};
use ollama_rs::Ollama;
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

const OLLAMA_TIMEOUT: Duration = Duration::from_secs(300);

async fn generate_with_timeout(
    ollama: &Ollama,
    request: GenerationRequest<'_>,
) -> anyhow::Result<ollama_rs::generation::completion::GenerationResponse> {
    tokio::time::timeout(OLLAMA_TIMEOUT, ollama.generate(request))
        .await
        .map_err(|_| anyhow::anyhow!("Ollama request timed out after {:?}", OLLAMA_TIMEOUT))?
        .map_err(Into::into)
}

async fn embeddings_with_timeout(
    ollama: &Ollama,
    request: GenerateEmbeddingsRequest,
) -> anyhow::Result<ollama_rs::generation::embeddings::GenerateEmbeddingsResponse> {
    tokio::time::timeout(OLLAMA_TIMEOUT, ollama.generate_embeddings(request))
        .await
        .map_err(|_| anyhow::anyhow!("Ollama request timed out after {:?}", OLLAMA_TIMEOUT))?
        .map_err(Into::into)
}

pub async fn tags(ollama_url: &Url, ollama_model: &str, text: &str) -> anyhow::Result<Vec<String>> {
    const PROMPT_PREFIX: &str = r#"Extract up to 5 tags from this text slice.
    Each tag must be 1-2 words maximum (e.g., "rust", "async runtime", "postgresql").
    Focus on programming and technology topics.
    Avoid generic terms like "programming", "technology", "software", or "code".
    Only include tags that are specific and meaningful to this content.
    It's ok to return fewer tags or none if nothing specific stands out.
    Here's the text: "#;

    let ollama = Ollama::from_url(ollama_url.to_owned());
    let format = FormatType::StructuredJson(Box::new(JsonStructure::new::<TagsModelResponse>()));
    let request =
        GenerationRequest::new(ollama_model.to_owned(), format!("{PROMPT_PREFIX}\n{text}"))
            .format(format)
            .system(SYSTEM_PROMPT);
    let response = generate_with_timeout(&ollama, request).await?;
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
    const PROMPT_PREFIX: &str = r#"Consolidate these tags into a final list of maximum 7 tags.
    Merge synonyms and related concepts into the most specific term.
    Each tag must be 1-2 words maximum. Remove anything generic or redundant.
    Prefer specific terms over broad ones (e.g., "tokio" over "async", "react hooks" over "frontend").
    Here are the tags: "#;

    let text = tags.join(", ");
    let ollama = Ollama::from_url(ollama_url.to_owned());
    let format = FormatType::StructuredJson(Box::new(JsonStructure::new::<TagsModelResponse>()));
    let request =
        GenerationRequest::new(ollama_model.to_owned(), format!("{PROMPT_PREFIX}\n{text}"))
            .format(format)
            .system(SYSTEM_PROMPT);
    let response = generate_with_timeout(&ollama, request).await?;
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
    const PROMPT_PREFIX: &str = r#"The following text is a slice of a bigger article.
    I'm asking you to produce a short summary of it that better describes this slice of text.
    Most of the text will be related to programming and technology, so focus on that.
    Avoid topics that are too broad or generic, try to focus on what will make this piece distinct.
    It's ok to not provide any summary if you think there's nothing relevant to point out.
    Try to be very succinct, one sentence would be the ideal.
    Here's the text: "#;

    let ollama = Ollama::from_url(ollama_url.to_owned());

    let format = FormatType::StructuredJson(Box::new(JsonStructure::new::<SummaryModelResponse>()));
    let request =
        GenerationRequest::new(ollama_model.to_owned(), format!("{PROMPT_PREFIX}\n{text}"))
            .format(format)
            .system(SYSTEM_PROMPT);
    let response = generate_with_timeout(&ollama, request).await?;
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
    Most of the summaries are related to programming and technology.
    Most of the summaries look like duplicated, redundant or ambiguous.
    Try to produce a new consolidated summary of them all, that is more clear and succinct.
    Less is better, give me maximum of 3 sentences.
    Here are the summaries, separated by new lines: "#;

    let text = summaries.join("\n");
    let ollama = Ollama::from_url(ollama_url.to_owned());
    let format = FormatType::StructuredJson(Box::new(JsonStructure::new::<SummaryModelResponse>()));
    let request =
        GenerationRequest::new(ollama_model.to_owned(), format!("{PROMPT_PREFIX}\n{text}"))
            .format(format)
            .system(SYSTEM_PROMPT);
    let response = generate_with_timeout(&ollama, request).await?;
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

pub async fn embeddings(
    ollama_url: &Url,
    ollama_model: &str,
    text: &str,
) -> anyhow::Result<Vec<f32>> {
    let ollama = Ollama::from_url(ollama_url.to_owned());
    let input = EmbeddingsInput::Single(text.to_string());
    let request = GenerateEmbeddingsRequest::new(ollama_model.to_owned(), input);
    let response = embeddings_with_timeout(&ollama, request).await?;
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

pub async fn generate_similar_questions(
    ollama_url: &Url,
    ollama_model: &str,
    question: &str,
) -> anyhow::Result<Vec<String>> {
    const PROMPT_PREFIX: &str = r#"Given the following question, generate 4 additional similar questions that would help find the same or related information. The questions should be variations with different phrasings, perspectives, or levels of specificity.

Original question: "#;

    #[derive(JsonSchema, Deserialize)]
    struct QuestionsResponse {
        questions: Vec<String>,
    }

    let ollama = Ollama::from_url(ollama_url.to_owned());
    let format = FormatType::StructuredJson(Box::new(JsonStructure::new::<QuestionsResponse>()));
    let request = GenerationRequest::new(
        ollama_model.to_owned(),
        format!("{PROMPT_PREFIX}\n{question}"),
    )
    .format(format)
    .system(SYSTEM_PROMPT);
    let response = generate_with_timeout(&ollama, request).await?;
    if let Ok(parsed) = serde_json::from_str::<QuestionsResponse>(&response.response) {
        Ok(parsed.questions)
    } else {
        bail!(
            "Ollama failed to generate similar questions, question: {}, response: {}",
            question,
            response.response
        )
    }
}

pub async fn assess_chunk_relevance(
    ollama_url: &Url,
    ollama_model: &str,
    question: &str,
    chunk_text: &str,
) -> anyhow::Result<(bool, String)> {
    const PROMPT_PREFIX: &str = r#"Assess whether the given text chunk is relevant to answering the question. 

Question: {question}

Text chunk: {chunk}

Evaluate if this chunk contains information that would help answer the question. Respond with:
- relevant: true/false
- explanation: brief explanation of why it is or isn't relevant"#;

    #[derive(JsonSchema, Deserialize)]
    struct RelevanceResponse {
        relevant: bool,
        explanation: String,
    }

    let ollama = Ollama::from_url(ollama_url.to_owned());
    let format = FormatType::StructuredJson(Box::new(JsonStructure::new::<RelevanceResponse>()));
    let prompt = PROMPT_PREFIX
        .replace("{question}", question)
        .replace("{chunk}", chunk_text);
    let request = GenerationRequest::new(ollama_model.to_owned(), prompt)
        .format(format)
        .system(SYSTEM_PROMPT);
    let response = generate_with_timeout(&ollama, request).await?;
    if let Ok(parsed) = serde_json::from_str::<RelevanceResponse>(&response.response) {
        Ok((parsed.relevant, parsed.explanation))
    } else {
        bail!(
            "Ollama failed to assess chunk relevance, question: {}, chunk: {}, response: {}",
            question,
            chunk_text,
            response.response
        )
    }
}

pub async fn answer_with_context(
    ollama_url: &Url,
    ollama_model: &str,
    question: &str,
    context_chunks: &[String],
) -> anyhow::Result<String> {
    let context = context_chunks.join("\n\n");
    let prompt = format!(
        r#"Given this context information, answer the following question. If the context doesn't contain enough information to answer the question, say so clearly.

Context:
{}

Question: {}

Provide a clear, accurate answer based on the context provided. If you cannot answer based on the context, explain what information would be needed."#,
        context, question
    );

    let ollama = Ollama::from_url(ollama_url.to_owned());
    let request = GenerationRequest::new(ollama_model.to_owned(), prompt).system(SYSTEM_PROMPT);
    let response = generate_with_timeout(&ollama, request).await?;
    Ok(response.response)
}
