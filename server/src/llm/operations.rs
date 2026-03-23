use anyhow::Result;
use rig::client::{CompletionClient, EmbeddingsClient};
use rig::completion::Prompt;
use serde::{Deserialize, Serialize};

use super::{EmbeddingClient, LlmClient, TextClient};

#[derive(Deserialize, Serialize, schemars::JsonSchema)]
struct TagsModelResponse {
    tags: Vec<String>,
}

#[derive(Deserialize, Serialize, schemars::JsonSchema)]
struct SummaryModelResponse {
    summary: String,
}

#[derive(Deserialize, Serialize, schemars::JsonSchema)]
struct QuestionsResponse {
    questions: Vec<String>,
}

#[derive(Deserialize, Serialize, schemars::JsonSchema)]
struct RelevanceResponse {
    relevant: bool,
    explanation: String,
}

const SYSTEM_PROMPT: &str = r#"You are an expert researcher. Follow these instructions when responding:
  - The user is a highly experienced analyst, no need to simplify it, be as detailed as possible and make sure your response is correct.
  - Be highly organized.
  - Treat me as an expert in all subject matter.
  - Mistakes erode my trust, so be accurate and thorough.
  - Be succinct and cohesive.
"#;

/// Dispatch a structured extraction call across all text provider variants.
/// Each arm builds an Extractor<ProviderModel, T> and calls .extract().
macro_rules! extract {
    ($client:expr, $model:expr, $preamble:expr, $prompt:expr, $T:ty) => {{
        let preamble = $preamble;
        let model = $model;
        let prompt = $prompt;
        let result: Result<$T> = match $client {
            TextClient::Ollama(c) => {
                let e = c.extractor::<$T>(model).preamble(preamble).build();
                e.extract(prompt).await.map_err(anyhow::Error::from)
            }
            TextClient::OpenAI(c) => {
                let e = c.extractor::<$T>(model).preamble(preamble).build();
                e.extract(prompt).await.map_err(anyhow::Error::from)
            }
            TextClient::Anthropic(c) => {
                let e = c.extractor::<$T>(model).preamble(preamble).build();
                e.extract(prompt).await.map_err(anyhow::Error::from)
            }
            TextClient::Gemini(c) => {
                let e = c.extractor::<$T>(model).preamble(preamble).build();
                e.extract(prompt).await.map_err(anyhow::Error::from)
            }
            TextClient::OpenRouter(c) => {
                let e = c.extractor::<$T>(model).preamble(preamble).build();
                e.extract(prompt).await.map_err(anyhow::Error::from)
            }
        };
        result
    }};
}

/// Dispatch a plain text prompt across all text provider variants.
macro_rules! prompt {
    ($client:expr, $model:expr, $preamble:expr, $prompt:expr) => {{
        let preamble = $preamble;
        let model = $model;
        let prompt = $prompt;
        let result: Result<String> = match $client {
            TextClient::Ollama(c) => {
                let a = c.agent(model).preamble(preamble).build();
                a.prompt(prompt).await.map_err(anyhow::Error::from)
            }
            TextClient::OpenAI(c) => {
                let a = c.agent(model).preamble(preamble).build();
                a.prompt(prompt).await.map_err(anyhow::Error::from)
            }
            TextClient::Anthropic(c) => {
                let a = c.agent(model).preamble(preamble).build();
                a.prompt(prompt).await.map_err(anyhow::Error::from)
            }
            TextClient::Gemini(c) => {
                let a = c.agent(model).preamble(preamble).build();
                a.prompt(prompt).await.map_err(anyhow::Error::from)
            }
            TextClient::OpenRouter(c) => {
                let a = c.agent(model).preamble(preamble).build();
                a.prompt(prompt).await.map_err(anyhow::Error::from)
            }
        };
        result
    }};
}

pub async fn tags(client: &LlmClient, text: &str) -> Result<Vec<String>> {
    const PROMPT_PREFIX: &str = r#"Extract up to 5 tags from this text slice.
    Each tag must be 1-2 words maximum (e.g., "rust", "async runtime", "postgresql").
    Focus on programming and technology topics.
    Avoid generic terms like "programming", "technology", "software", or "code".
    Only include tags that are specific and meaningful to this content.
    It's ok to return fewer tags or none if nothing specific stands out.
    Here's the text: "#;

    let prompt = format!("{PROMPT_PREFIX}\n{text}");
    let resp: TagsModelResponse = extract!(
        &client.text_client,
        &client.text_model,
        SYSTEM_PROMPT,
        &prompt,
        TagsModelResponse
    )?;
    Ok(resp.tags)
}

pub async fn consolidate_tags(client: &LlmClient, tags: Vec<String>) -> Result<Vec<String>> {
    const PROMPT_PREFIX: &str = r#"Consolidate these tags into a final list of maximum 7 tags.
    Merge synonyms and related concepts into the most specific term.
    Each tag must be 1-2 words maximum. Remove anything generic or redundant.
    Prefer specific terms over broad ones (e.g., "tokio" over "async", "react hooks" over "frontend").
    Here are the tags: "#;

    let text = tags.join(", ");
    let prompt = format!("{PROMPT_PREFIX}\n{text}");
    let resp: TagsModelResponse = extract!(
        &client.text_client,
        &client.text_model,
        SYSTEM_PROMPT,
        &prompt,
        TagsModelResponse
    )?;
    Ok(resp.tags)
}

pub async fn summary(client: &LlmClient, text: &str) -> Result<String> {
    const PROMPT_PREFIX: &str = r#"The following text is a slice of a bigger article.
    I'm asking you to produce a short summary of it that better describes this slice of text.
    Most of the text will be related to programming and technology, so focus on that.
    Avoid topics that are too broad or generic, try to focus on what will make this piece distinct.
    It's ok to not provide any summary if you think there's nothing relevant to point out.
    Try to be very succinct, one sentence would be the ideal.
    Here's the text: "#;

    let prompt = format!("{PROMPT_PREFIX}\n{text}");
    let resp: SummaryModelResponse = extract!(
        &client.text_client,
        &client.text_model,
        SYSTEM_PROMPT,
        &prompt,
        SummaryModelResponse
    )?;
    Ok(resp.summary)
}

pub async fn consolidate_summary(client: &LlmClient, summaries: &[String]) -> Result<String> {
    const PROMPT_PREFIX: &str = r#"I'll give you a list of summaries, they come from slices of an article.
    Most of the summaries are related to programming and technology.
    Most of the summaries look like duplicated, redundant or ambiguous.
    Try to produce a new consolidated summary of them all, that is more clear and succinct.
    Less is better, give me maximum of 3 sentences.
    Here are the summaries, separated by new lines: "#;

    let text = summaries.join("\n");
    let prompt = format!("{PROMPT_PREFIX}\n{text}");
    let resp: SummaryModelResponse = extract!(
        &client.text_client,
        &client.text_model,
        SYSTEM_PROMPT,
        &prompt,
        SummaryModelResponse
    )?;
    Ok(resp.summary)
}

pub async fn embeddings(client: &LlmClient, text: &str) -> Result<Vec<f32>> {
    use rig::embeddings::EmbeddingModel;

    let embedding = match &client.embedding_client {
        EmbeddingClient::Ollama(c) => {
            let model =
                c.embedding_model_with_ndims(&client.embedding_model, client.embedding_ndims);
            model.embed_text(text).await?
        }
        EmbeddingClient::OpenAI(c) => {
            let model =
                c.embedding_model_with_ndims(&client.embedding_model, client.embedding_ndims);
            model.embed_text(text).await?
        }
        EmbeddingClient::Gemini(c) => {
            let model =
                c.embedding_model_with_ndims(&client.embedding_model, client.embedding_ndims);
            model.embed_text(text).await?
        }
    };
    // rig returns Vec<f64>, pgvector expects Vec<f32>
    Ok(embedding.vec.into_iter().map(|v| v as f32).collect())
}

pub async fn generate_similar_questions(client: &LlmClient, question: &str) -> Result<Vec<String>> {
    const PROMPT_PREFIX: &str = r#"Given the following question, generate 4 additional similar questions that would help find the same or related information. The questions should be variations with different phrasings, perspectives, or levels of specificity.

Original question: "#;

    let prompt = format!("{PROMPT_PREFIX}\n{question}");
    let resp: QuestionsResponse = extract!(
        &client.text_client,
        &client.text_model,
        SYSTEM_PROMPT,
        &prompt,
        QuestionsResponse
    )?;
    Ok(resp.questions)
}

pub async fn assess_chunk_relevance(
    client: &LlmClient,
    question: &str,
    chunk_text: &str,
) -> Result<(bool, String)> {
    const PROMPT_TEMPLATE: &str = r#"Assess whether the given text chunk is relevant to answering the question.

Question: {question}

Text chunk: {chunk}

Evaluate if this chunk contains information that would help answer the question. Respond with:
- relevant: true/false
- explanation: brief explanation of why it is or isn't relevant"#;

    let prompt = PROMPT_TEMPLATE
        .replace("{question}", question)
        .replace("{chunk}", chunk_text);
    let resp: RelevanceResponse = extract!(
        &client.text_client,
        &client.text_model,
        SYSTEM_PROMPT,
        &prompt,
        RelevanceResponse
    )?;
    Ok((resp.relevant, resp.explanation))
}

pub async fn answer_with_context(
    client: &LlmClient,
    question: &str,
    context_chunks: &[String],
) -> Result<String> {
    let context = context_chunks.join("\n\n");
    let user_prompt = format!(
        r#"Given this context information, answer the following question. If the context doesn't contain enough information to answer the question, say so clearly.

Context:
{}

Question: {}

Provide a clear, accurate answer based on the context provided. If you cannot answer based on the context, explain what information would be needed."#,
        context, question
    );

    prompt!(
        &client.text_client,
        &client.text_model,
        SYSTEM_PROMPT,
        &user_prompt
    )
}
