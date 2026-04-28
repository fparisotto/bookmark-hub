use anyhow::Result;
use rig::client::{CompletionClient, EmbeddingsClient};
use rig::completion::Prompt;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use super::{EmbeddingClient, LlmClient, LlmRequestKind, LlmWorkClass, TextClient};

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct CombinedChunkAnalysis {
    pub summary: String,
    pub tags: Vec<String>,
}

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

async fn extract_structured<T>(
    client: &LlmClient,
    class: LlmWorkClass,
    preamble: &str,
    prompt: &str,
    operation_name: &str,
) -> Result<T>
where
    T: DeserializeOwned + Serialize + schemars::JsonSchema + Send + Sync + 'static,
{
    client
        .run_with_retry(class, LlmRequestKind::Text, operation_name, || async {
            let result: Result<T> = match &client.text_client {
                TextClient::Ollama(c) => {
                    let e = c
                        .extractor::<T>(&client.text_model)
                        .preamble(preamble)
                        .build();
                    e.extract(prompt).await.map_err(anyhow::Error::from)
                }
                TextClient::OpenAI(c) => {
                    let e = c
                        .extractor::<T>(&client.text_model)
                        .preamble(preamble)
                        .build();
                    e.extract(prompt).await.map_err(anyhow::Error::from)
                }
                TextClient::Anthropic(c) => {
                    let e = c
                        .extractor::<T>(&client.text_model)
                        .preamble(preamble)
                        .build();
                    e.extract(prompt).await.map_err(anyhow::Error::from)
                }
                TextClient::Gemini(c) => {
                    let e = c
                        .extractor::<T>(&client.text_model)
                        .preamble(preamble)
                        .build();
                    e.extract(prompt).await.map_err(anyhow::Error::from)
                }
                TextClient::OpenRouter(c) => {
                    let e = c
                        .extractor::<T>(&client.text_model)
                        .preamble(preamble)
                        .build();
                    e.extract(prompt).await.map_err(anyhow::Error::from)
                }
            };
            result
        })
        .await
}

async fn prompt_text(
    client: &LlmClient,
    class: LlmWorkClass,
    preamble: &str,
    prompt: &str,
    operation_name: &str,
) -> Result<String> {
    client
        .run_with_retry(class, LlmRequestKind::Text, operation_name, || async {
            match &client.text_client {
                TextClient::Ollama(c) => {
                    let a = c.agent(&client.text_model).preamble(preamble).build();
                    a.prompt(prompt).await.map_err(anyhow::Error::from)
                }
                TextClient::OpenAI(c) => {
                    let a = c.agent(&client.text_model).preamble(preamble).build();
                    a.prompt(prompt).await.map_err(anyhow::Error::from)
                }
                TextClient::Anthropic(c) => {
                    let a = c.agent(&client.text_model).preamble(preamble).build();
                    a.prompt(prompt).await.map_err(anyhow::Error::from)
                }
                TextClient::Gemini(c) => {
                    let a = c.agent(&client.text_model).preamble(preamble).build();
                    a.prompt(prompt).await.map_err(anyhow::Error::from)
                }
                TextClient::OpenRouter(c) => {
                    let a = c.agent(&client.text_model).preamble(preamble).build();
                    a.prompt(prompt).await.map_err(anyhow::Error::from)
                }
            }
        })
        .await
}

pub async fn analyze_chunk(client: &LlmClient, text: &str) -> Result<CombinedChunkAnalysis> {
    const PROMPT_PREFIX: &str = r#"The following text is a slice of a larger article.
Return:
- a short summary that captures what is distinctive in this slice
- up to 5 specific tags

Rules:
- focus on programming and technology topics
- avoid generic tags like "software", "programming", or "technology"
- tags must be 1-2 words
- the summary should be concise, ideally one sentence
- it is acceptable to return an empty summary or no tags if nothing specific stands out

Text:"#;

    extract_structured(
        client,
        LlmWorkClass::Background,
        SYSTEM_PROMPT,
        &format!("{PROMPT_PREFIX}\n{text}"),
        "analyze_chunk",
    )
    .await
}

pub async fn consolidate_tags(client: &LlmClient, tags: Vec<String>) -> Result<Vec<String>> {
    const PROMPT_PREFIX: &str = r#"Consolidate these tags into a final list of maximum 7 tags.
Merge synonyms and related concepts into the most specific term.
Each tag must be 1-2 words maximum. Remove anything generic or redundant.
Prefer specific terms over broad ones (e.g., "tokio" over "async", "react hooks" over "frontend").
Here are the tags: "#;

    let text = tags.join(", ");
    let resp: TagsModelResponse = extract_structured(
        client,
        LlmWorkClass::Background,
        SYSTEM_PROMPT,
        &format!("{PROMPT_PREFIX}\n{text}"),
        "consolidate_tags",
    )
    .await?;
    Ok(resp.tags)
}

pub async fn consolidate_summary(client: &LlmClient, summaries: &[String]) -> Result<String> {
    const PROMPT_PREFIX: &str = r#"I'll give you a list of summaries, they come from slices of an article.
Most of the summaries are related to programming and technology.
Most of the summaries look duplicated, redundant or ambiguous.
Try to produce a new consolidated summary that is clearer and more succinct.
Less is better, give me a maximum of 3 sentences.
Here are the summaries, separated by new lines: "#;

    let text = summaries.join("\n");
    let resp: SummaryModelResponse = extract_structured(
        client,
        LlmWorkClass::Background,
        SYSTEM_PROMPT,
        &format!("{PROMPT_PREFIX}\n{text}"),
        "consolidate_summary",
    )
    .await?;
    Ok(resp.summary)
}

pub async fn embeddings_background(client: &LlmClient, text: &str) -> Result<Vec<f32>> {
    embeddings_with_dimensions(
        &client.embedding_client,
        &client.embedding_model,
        text,
        Some(client.embedding_ndims),
        client,
        LlmWorkClass::Background,
    )
    .await
}

pub async fn embeddings_interactive(client: &LlmClient, text: &str) -> Result<Vec<f32>> {
    embeddings_with_dimensions(
        &client.embedding_client,
        &client.embedding_model,
        text,
        Some(client.embedding_ndims),
        client,
        LlmWorkClass::Interactive,
    )
    .await
}

pub async fn embeddings_with_dimensions(
    client: &EmbeddingClient,
    model_name: &str,
    text: &str,
    dimensions: Option<usize>,
    owner: &LlmClient,
    class: LlmWorkClass,
) -> Result<Vec<f32>> {
    use rig::embeddings::EmbeddingModel;

    let embedding = owner
        .run_with_retry(class, LlmRequestKind::Embedding, "embeddings", || async {
            match (client, dimensions) {
                (EmbeddingClient::Ollama(c), Some(dimensions)) => c
                    .embedding_model_with_ndims(model_name, dimensions)
                    .embed_text(text)
                    .await
                    .map_err(anyhow::Error::from),
                (EmbeddingClient::Ollama(c), None) => c
                    .embedding_model(model_name)
                    .embed_text(text)
                    .await
                    .map_err(anyhow::Error::from),
                (EmbeddingClient::OpenAI(c), Some(dimensions)) => c
                    .embedding_model_with_ndims(model_name, dimensions)
                    .embed_text(text)
                    .await
                    .map_err(anyhow::Error::from),
                (EmbeddingClient::OpenAI(c), None) => c
                    .embedding_model(model_name)
                    .embed_text(text)
                    .await
                    .map_err(anyhow::Error::from),
                (EmbeddingClient::Gemini(c), Some(dimensions)) => c
                    .embedding_model_with_ndims(model_name, dimensions)
                    .embed_text(text)
                    .await
                    .map_err(anyhow::Error::from),
                (EmbeddingClient::Gemini(c), None) => c
                    .embedding_model(model_name)
                    .embed_text(text)
                    .await
                    .map_err(anyhow::Error::from),
            }
        })
        .await?;

    Ok(embedding.vec.into_iter().map(|v| v as f32).collect())
}

pub async fn generate_similar_questions(client: &LlmClient, question: &str) -> Result<Vec<String>> {
    const PROMPT_PREFIX: &str = r#"Given the following question, generate 4 additional similar questions that would help find the same or related information. The questions should be variations with different phrasings, perspectives, or levels of specificity.

Original question: "#;

    let resp: QuestionsResponse = extract_structured(
        client,
        LlmWorkClass::Interactive,
        SYSTEM_PROMPT,
        &format!("{PROMPT_PREFIX}\n{question}"),
        "generate_similar_questions",
    )
    .await?;
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
    let resp: RelevanceResponse = extract_structured(
        client,
        LlmWorkClass::Interactive,
        SYSTEM_PROMPT,
        &prompt,
        "assess_chunk_relevance",
    )
    .await?;
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

    prompt_text(
        client,
        LlmWorkClass::Interactive,
        SYSTEM_PROMPT,
        &user_prompt,
        "answer_with_context",
    )
    .await
}
