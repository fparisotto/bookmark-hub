use anyhow::{Context, Result};
use shared::{RagChunkMatch, RagQueryRequest, RagQueryResponse};
use tracing::{debug, info, warn};
use url::Url;
use uuid::Uuid;

use crate::db::chunks::search_similar_chunks;
use crate::db::rag::{create_rag_session, update_rag_session};
use crate::db::PgPool;
use crate::ollama;

const DEFAULT_MAX_CHUNKS: usize = 10;
const DEFAULT_SIMILARITY_THRESHOLD: f64 = 0.3;
const EMBEDDING_MODEL: &str = "nomic-embed-text:v1.5";

pub struct RagEngine {
    pool: PgPool,
    ollama_url: Url,
    text_model: String,
    embedding_model: String,
}

impl RagEngine {
    pub fn new(pool: PgPool, ollama_url: Url, text_model: String) -> Self {
        Self {
            pool,
            ollama_url,
            text_model,
            embedding_model: EMBEDDING_MODEL.to_string(),
        }
    }

    pub async fn process_query(
        &self,
        user_id: Uuid,
        request: &RagQueryRequest,
    ) -> Result<RagQueryResponse> {
        info!(
            user_id = %user_id,
            question = %request.question,
            "Processing RAG query"
        );

        // Create a new session for this query
        let session = create_rag_session(&self.pool, user_id, &request.question)
            .await
            .context("Failed to create RAG session")?;

        // Step 1: Question augmentation - generate similar questions
        let questions = self.generate_query_variations(&request.question).await?;

        // Step 2: Generate embeddings for all questions
        let mut all_matches = Vec::new();
        for question in questions {
            let matches = self
                .search_for_question(user_id, &question, request)
                .await?;
            all_matches.extend(matches);
        }

        // Remove duplicates and sort by similarity
        all_matches.sort_by(|a, b| b.similarity_score.partial_cmp(&a.similarity_score).unwrap());
        all_matches.dedup_by(|a, b| a.chunk.chunk_id == b.chunk.chunk_id);

        // Limit the number of chunks
        let max_chunks = request.max_chunks.unwrap_or(DEFAULT_MAX_CHUNKS);
        all_matches.truncate(max_chunks);

        info!(
            user_id = %user_id,
            session_id = %session.session_id,
            matches_found = all_matches.len(),
            "Found similar chunks"
        );

        // Step 3: Assess relevance of each chunk
        let relevant_matches = self
            .assess_chunk_relevance(&request.question, all_matches)
            .await?;

        // Step 4: Generate answer using relevant chunks
        let answer = if relevant_matches.is_empty() {
            "I couldn't find any relevant information in your bookmarks to answer this question."
                .to_string()
        } else {
            let context_chunks: Vec<String> = relevant_matches
                .iter()
                .map(|m| m.chunk.chunk_text.clone())
                .collect();

            self.generate_answer(&request.question, &context_chunks)
                .await?
        };

        // Step 5: Update session with answer and relevant chunks
        let relevant_chunk_ids: Vec<Uuid> =
            relevant_matches.iter().map(|m| m.chunk.chunk_id).collect();

        let updated_session = update_rag_session(
            &self.pool,
            session.session_id,
            user_id,
            &answer,
            &relevant_chunk_ids,
        )
        .await
        .context("Failed to update RAG session")?;

        info!(
            user_id = %user_id,
            session_id = %session.session_id,
            relevant_chunks = relevant_matches.len(),
            "RAG query processed successfully"
        );

        Ok(RagQueryResponse {
            session_id: updated_session.session_id,
            question: request.question.clone(),
            answer,
            relevant_chunks: relevant_matches,
            created_at: updated_session.created_at,
        })
    }

    async fn generate_query_variations(&self, question: &str) -> Result<Vec<String>> {
        let mut questions = vec![question.to_string()];

        match ollama::generate_similar_questions(&self.ollama_url, &self.text_model, question).await
        {
            Ok(similar_questions) => {
                questions.extend(similar_questions);
                debug!(
                    original_question = %question,
                    total_questions = questions.len(),
                    "Generated question variations"
                );
            }
            Err(error) => {
                warn!(
                    ?error,
                    "Failed to generate similar questions, using original only"
                );
            }
        }

        Ok(questions)
    }

    async fn search_for_question(
        &self,
        user_id: Uuid,
        question: &str,
        request: &RagQueryRequest,
    ) -> Result<Vec<RagChunkMatch>> {
        // Generate embedding for the question
        let query_embedding = ollama::embeddings(&self.ollama_url, &self.embedding_model, question)
            .await
            .context("Failed to generate embedding for question")?;

        // Search for similar chunks
        let similarity_threshold = request
            .similarity_threshold
            .unwrap_or(DEFAULT_SIMILARITY_THRESHOLD);
        let max_chunks = request.max_chunks.unwrap_or(DEFAULT_MAX_CHUNKS);

        search_similar_chunks(
            &self.pool,
            user_id,
            query_embedding,
            max_chunks * 2, // Get more chunks than needed for relevance filtering
            similarity_threshold,
        )
        .await
        .context("Failed to search for similar chunks")
    }

    async fn assess_chunk_relevance(
        &self,
        question: &str,
        matches: Vec<RagChunkMatch>,
    ) -> Result<Vec<RagChunkMatch>> {
        let mut relevant_matches = Vec::new();

        let match_count = matches.len();
        for mut chunk_match in matches {
            match ollama::assess_chunk_relevance(
                &self.ollama_url,
                &self.text_model,
                question,
                &chunk_match.chunk.chunk_text,
            )
            .await
            {
                Ok((is_relevant, explanation)) => {
                    if is_relevant {
                        let chunk_id = chunk_match.chunk.chunk_id;
                        let similarity_score = chunk_match.similarity_score;
                        chunk_match.relevance_explanation = Some(explanation);
                        relevant_matches.push(chunk_match);
                        debug!(
                            chunk_id = %chunk_id,
                            similarity_score = similarity_score,
                            "Chunk assessed as relevant"
                        );
                    } else {
                        debug!(
                            chunk_id = %chunk_match.chunk.chunk_id,
                            similarity_score = chunk_match.similarity_score,
                            explanation = %explanation,
                            "Chunk assessed as not relevant"
                        );
                    }
                }
                Err(error) => {
                    warn!(
                        chunk_id = %chunk_match.chunk.chunk_id,
                        ?error,
                        "Failed to assess chunk relevance, including by default"
                    );
                    // Include chunk if relevance assessment fails
                    chunk_match.relevance_explanation =
                        Some("Could not assess relevance".to_string());
                    relevant_matches.push(chunk_match);
                }
            }
        }

        // Sort by similarity score (highest first)
        relevant_matches
            .sort_by(|a, b| b.similarity_score.partial_cmp(&a.similarity_score).unwrap());

        info!(
            total_matches = match_count,
            relevant_matches = relevant_matches.len(),
            "Completed chunk relevance assessment"
        );

        Ok(relevant_matches)
    }

    async fn generate_answer(&self, question: &str, context_chunks: &[String]) -> Result<String> {
        ollama::answer_with_context(&self.ollama_url, &self.text_model, question, context_chunks)
            .await
            .context("Failed to generate answer with context")
    }
}
