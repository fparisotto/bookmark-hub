use anyhow::{Context, Result};
use shared::{RagChunkMatch, RagQueryRequest, RagQueryResponse};
use tracing::{debug, info, warn};
use url::Url;
use uuid::Uuid;

use crate::db::chunks::{search_chunks_hybrid, search_similar_chunks, HybridChunkMatch};
use crate::db::rag::{create_rag_session, update_rag_session};
use crate::db::PgPool;
use crate::ollama;
use crate::tokenizer::count_tokens;

const DEFAULT_MAX_CHUNKS: usize = 6;
const DEFAULT_SIMILARITY_THRESHOLD: f64 = 0.3;
const EMBEDDING_MODEL: &str = "mxbai-embed-large";
const DEFAULT_MAX_CONTEXT_TOKENS: usize = 4096;
const PROMPT_OVERHEAD_TOKENS: usize = 200;
const DEFAULT_RRF_K: u32 = 60;

/// Reciprocal Rank Fusion score
/// RRF(d) = 1/(k + rank_vector) + 1/(k + rank_fts)
fn rrf_score(vector_rank: usize, fts_rank: usize, k: u32) -> f64 {
    1.0 / (k as f64 + vector_rank as f64) + 1.0 / (k as f64 + fts_rank as f64)
}

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

        // Step 4: Apply token budget to select chunks that fit within context limits
        let max_tokens = request
            .max_context_tokens
            .unwrap_or(DEFAULT_MAX_CONTEXT_TOKENS);
        let budgeted_matches = self.select_chunks_within_budget(relevant_matches, max_tokens)?;

        info!(
            user_id = %user_id,
            session_id = %session.session_id,
            budgeted_chunks = budgeted_matches.len(),
            max_tokens,
            "Applied token budget to chunks"
        );

        // Step 5: Generate answer using budgeted chunks
        let answer = if budgeted_matches.is_empty() {
            "I couldn't find any relevant information in your bookmarks to answer this question."
                .to_string()
        } else {
            let context_chunks: Vec<String> = budgeted_matches
                .iter()
                .map(|m| m.chunk.chunk_text.clone())
                .collect();

            self.generate_answer(&request.question, &context_chunks)
                .await?
        };

        // Step 6: Update session with answer and relevant chunks
        let relevant_chunk_ids: Vec<Uuid> =
            budgeted_matches.iter().map(|m| m.chunk.chunk_id).collect();

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
            relevant_chunks = budgeted_matches.len(),
            "RAG query processed successfully"
        );

        Ok(RagQueryResponse {
            session_id: updated_session.session_id,
            question: request.question.clone(),
            answer,
            relevant_chunks: budgeted_matches,
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

        // Check if hybrid search is enabled
        let use_hybrid = request
            .hybrid_search
            .as_ref()
            .map(|c| c.enabled)
            .unwrap_or(false);

        if use_hybrid {
            let hybrid_matches = search_chunks_hybrid(
                &self.pool,
                user_id,
                question,
                query_embedding,
                max_chunks * 2,
                similarity_threshold,
            )
            .await
            .context("Failed to search for chunks with hybrid search")?;

            let config = request.hybrid_search.as_ref().unwrap();
            let use_rrf = config.use_rrf.unwrap_or(true);

            if use_rrf {
                let k = config.rrf_k.unwrap_or(DEFAULT_RRF_K);
                Ok(self.combine_with_rrf(hybrid_matches, k))
            } else {
                let vector_weight = config.vector_weight.unwrap_or(0.5);
                let fts_weight = config.fts_weight.unwrap_or(0.5);
                Ok(self.combine_with_weights(hybrid_matches, vector_weight, fts_weight))
            }
        } else {
            // Existing vector-only search path
            let matches = search_similar_chunks(
                &self.pool,
                user_id,
                query_embedding,
                max_chunks * 2,
                similarity_threshold,
            )
            .await
            .context("Failed to search for similar chunks")?;

            // Add None for hybrid scores in vector-only mode
            Ok(matches
                .into_iter()
                .map(|m| RagChunkMatch {
                    chunk: m.chunk,
                    bookmark: m.bookmark,
                    similarity_score: m.similarity_score,
                    relevance_explanation: m.relevance_explanation,
                    vector_score: None,
                    fts_score: None,
                    combined_score: None,
                })
                .collect())
        }
    }

    /// Combine hybrid search results using Reciprocal Rank Fusion
    /// RRF(d) = 1/(k + rank_vector) + 1/(k + rank_fts)
    fn combine_with_rrf(&self, matches: Vec<HybridChunkMatch>, k: u32) -> Vec<RagChunkMatch> {
        let mut results: Vec<RagChunkMatch> = matches
            .into_iter()
            .map(|m| {
                let rrf_score = rrf_score(m.vector_rank, m.fts_rank, k);
                RagChunkMatch {
                    chunk: m.chunk,
                    bookmark: m.bookmark,
                    similarity_score: rrf_score, // Use RRF as primary score for sorting
                    relevance_explanation: None,
                    vector_score: Some(m.vector_score),
                    fts_score: Some(m.fts_score),
                    combined_score: Some(rrf_score),
                }
            })
            .collect();

        // Sort by combined RRF score (highest first)
        results.sort_by(|a, b| {
            b.combined_score
                .partial_cmp(&a.combined_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        debug!(
            results_count = results.len(),
            k, "Combined hybrid results with RRF"
        );

        results
    }

    /// Combine hybrid search results using weighted average
    fn combine_with_weights(
        &self,
        matches: Vec<HybridChunkMatch>,
        vector_weight: f64,
        fts_weight: f64,
    ) -> Vec<RagChunkMatch> {
        let total_weight = vector_weight + fts_weight;
        let norm_vector_weight = vector_weight / total_weight;
        let norm_fts_weight = fts_weight / total_weight;

        let mut results: Vec<RagChunkMatch> = matches
            .into_iter()
            .map(|m| {
                let combined = m.vector_score * norm_vector_weight + m.fts_score * norm_fts_weight;
                RagChunkMatch {
                    chunk: m.chunk,
                    bookmark: m.bookmark,
                    similarity_score: combined,
                    relevance_explanation: None,
                    vector_score: Some(m.vector_score),
                    fts_score: Some(m.fts_score),
                    combined_score: Some(combined),
                }
            })
            .collect();

        results.sort_by(|a, b| {
            b.combined_score
                .partial_cmp(&a.combined_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        debug!(
            results_count = results.len(),
            vector_weight = norm_vector_weight,
            fts_weight = norm_fts_weight,
            "Combined hybrid results with weighted average"
        );

        results
    }

    /// Select chunks that fit within the token budget
    fn select_chunks_within_budget(
        &self,
        chunks: Vec<RagChunkMatch>,
        max_tokens: usize,
    ) -> Result<Vec<RagChunkMatch>> {
        let available_tokens = max_tokens.saturating_sub(PROMPT_OVERHEAD_TOKENS);
        let mut selected = Vec::new();
        let mut total_tokens = 0;

        for chunk in chunks {
            let chunk_tokens = count_tokens(&chunk.chunk.chunk_text).unwrap_or_else(|e| {
                warn!(
                    chunk_id = %chunk.chunk.chunk_id,
                    error = %e,
                    "Failed to count tokens, estimating from char count"
                );
                // Rough estimate: ~4 chars per token for English text
                chunk.chunk.chunk_text.len() / 4
            });

            if total_tokens + chunk_tokens <= available_tokens {
                total_tokens += chunk_tokens;
                selected.push(chunk);
            } else {
                debug!(
                    chunk_id = %chunk.chunk.chunk_id,
                    chunk_tokens,
                    total_tokens,
                    available_tokens,
                    "Chunk exceeds token budget, stopping selection"
                );
                break;
            }
        }

        debug!(
            selected_count = selected.len(),
            total_tokens, available_tokens, "Selected chunks within token budget"
        );

        Ok(selected)
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
