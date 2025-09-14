use shared::{RagChunkMatch, RagQueryResponse};
use web_sys::HtmlTextAreaElement;
use yew::prelude::*;

use crate::components::atoms::markdown_render::MarkdownRender;

#[derive(Properties, PartialEq)]
pub struct RagQueryProps {
    pub question: String,
    pub response: Option<RagQueryResponse>,
    pub is_loading: bool,
    pub on_question_change: Callback<String>,
    pub on_submit: Callback<()>,
}

#[function_component(RagQuery)]
pub fn rag_query(props: &RagQueryProps) -> Html {
    let question_ref = use_node_ref();

    let on_input = {
        let on_question_change = props.on_question_change.clone();
        let question_ref = question_ref.clone();
        Callback::from(move |_: InputEvent| {
            if let Some(input) = question_ref.cast::<HtmlTextAreaElement>() {
                on_question_change.emit(input.value());
            }
        })
    };

    let on_submit = {
        let on_submit = props.on_submit.clone();
        let is_loading = props.is_loading;
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            if !is_loading {
                on_submit.emit(());
            }
        })
    };

    let on_keydown = {
        let on_submit = props.on_submit.clone();
        let is_loading = props.is_loading;
        Callback::from(move |e: KeyboardEvent| {
            if e.key() == "Enter" && (e.ctrl_key() || e.meta_key()) && !is_loading {
                e.prevent_default();
                on_submit.emit(());
            }
        })
    };

    html! {
        <div>
            <form class="mb-4" onsubmit={on_submit}>
                <div class="mb-3">
                    <label for="question" class="form-label">{"Your Question"}</label>
                    <textarea
                        ref={question_ref}
                        id="question"
                        rows="4"
                        class="form-control"
                        placeholder="Ask a question about your bookmarks... (Ctrl+Enter to submit)"
                        value={props.question.clone()}
                        oninput={on_input}
                        onkeydown={on_keydown}
                        disabled={props.is_loading}
                    />
                    <div class="form-text">{"💡 Tip: Use Ctrl+Enter to submit quickly"}</div>
                </div>
                <button
                    type="submit"
                    disabled={props.is_loading || props.question.trim().is_empty()}
                    class="btn btn-primary"
                >
                {
                    if props.is_loading {
                        html! {
                            <>
                                <span class="spinner-border spinner-border-sm me-2" role="status"></span>
                                {"Searching..."}
                            </>
                        }
                    } else {
                        html! { {"Ask AI"} }
                    }
                }
                </button>
            </form>

            // Response Display
            if let Some(response) = &props.response {
                <div>
                    // Answer
                    <div class="card mb-4">
                        <div class="card-header">
                            <h5 class="mb-0">{"🤖 Answer"}</h5>
                        </div>
                        <div class="card-body">
                            <MarkdownRender content={response.answer.clone()} />
                        </div>
                    </div>

                    // Sources
                    if !response.relevant_chunks.is_empty() {
                        <div class="card">
                            <div class="card-header">
                                <h5 class="mb-0">
                                    {"📚 Sources "}
                                    <small class="text-muted">
                                        {"(" }{response.relevant_chunks.len()}{" relevant chunks found)"}
                                    </small>
                                </h5>
                            </div>
                            <div class="card-body">
                                {
                                    response.relevant_chunks.iter().enumerate().map(|(index, chunk_match)| {
                                        render_chunk_match(index, chunk_match)
                                    }).collect::<Html>()
                                }
                            </div>
                        </div>
                    }
                </div>
            }
        </div>
    }
}

fn render_chunk_match(index: usize, chunk_match: &RagChunkMatch) -> Html {
    html! {
        <div class="border rounded mb-3 p-3">
            <div class="d-flex justify-content-between align-items-start mb-2">
                <div class="flex-grow-1">
                    <div class="d-flex align-items-center mb-2">
                        <span class="badge bg-primary me-2">
                            {"Source " }{index + 1}
                        </span>
                        <small class="text-muted">
                            {"Similarity: " }{format!("{:.1}%", chunk_match.similarity_score * 100.0)}
                        </small>
                    </div>
                    <h6 class="mb-1">
                        <a href={chunk_match.bookmark.url.clone()} target="_blank" rel="noopener noreferrer" class="text-decoration-none">
                            {&chunk_match.bookmark.title}
                        </a>
                    </h6>
                    <small class="text-muted">{&chunk_match.bookmark.domain}</small>
                </div>
            </div>

            <div class="border-start border-primary border-3 ps-3 mb-2">
                <MarkdownRender content={chunk_match.chunk.chunk_text.clone()} class={Some("small".to_string())} />
            </div>

            if let Some(explanation) = &chunk_match.relevance_explanation {
                <div class="mb-2">
                    <small class="text-muted fst-italic">
                        <strong>{"Relevance: "}</strong>{explanation}
                    </small>
                </div>
            }

            if let Some(ref tags) = chunk_match.bookmark.tags {
                if !tags.is_empty() {
                    <div>
                        {
                            tags.iter().map(|tag| html! {
                                <span class="badge bg-secondary me-1">
                                    {tag}
                                </span>
                            }).collect::<Html>()
                        }
                    </div>
                }
            }
        </div>
    }
}
