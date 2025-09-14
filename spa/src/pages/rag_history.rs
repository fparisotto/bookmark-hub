use chrono::{DateTime, Local};
use shared::{RagHistoryRequest, RagSession};
use yew::platform::spawn_local;
use yew::prelude::*;

use crate::api::rag_api;
use crate::components::atoms::markdown_render::MarkdownRender;
use crate::user_session::UserSession;

#[derive(Clone, PartialEq, Default, Debug)]
pub struct RagHistoryState {
    pub user_session: UserSession,
    pub history: Vec<RagSession>,
    pub is_loading: bool,
    pub error_message: Option<String>,
}

#[derive(Clone, PartialEq, Debug)]
pub enum RagHistoryMessage {
    SetUserSession(UserSession),
    LoadHistory,
    HistoryLoaded(Result<Vec<RagSession>, String>),
    ClearError,
}

#[derive(Properties, PartialEq)]
pub struct RagHistoryPageProps {
    pub user_session: UserSession,
}

#[function_component(RagHistoryPage)]
pub fn rag_history_page(props: &RagHistoryPageProps) -> Html {
    let state = use_reducer(|| RagHistoryState {
        user_session: props.user_session.clone(),
        is_loading: true,
        ..Default::default()
    });

    // Load history when component mounts
    use_effect_with((), {
        let state = state.clone();
        let user_session = props.user_session.clone();
        move |_| {
            let state = state.clone();
            let user_session = user_session.clone();
            spawn_local(async move {
                let request = RagHistoryRequest {
                    limit: Some(50),
                    offset: None,
                };

                let result = rag_api::get_rag_history(&user_session, &request).await;
                let action = match result {
                    Ok(response) => RagHistoryMessage::HistoryLoaded(Ok(response.sessions)),
                    Err(err) => RagHistoryMessage::HistoryLoaded(Err(format!(
                        "Failed to load history: {}",
                        err
                    ))),
                };

                state.dispatch(action);
            });
        }
    });

    let on_clear_error = {
        let state = state.clone();
        Callback::from(move |_| {
            state.dispatch(RagHistoryMessage::ClearError);
        })
    };

    html! {
        <div class="container mt-5">
            <div class="mb-4">
                <h1>{"AI Search History"}</h1>
                <p class="text-muted">{"View your previous AI-powered questions and answers."}</p>
            </div>

            // Error Display
            if let Some(error) = &state.error_message {
                <div class="alert alert-danger alert-dismissible" role="alert">
                    {error}
                    <button type="button" class="btn-close" onclick={on_clear_error}></button>
                </div>
            }

            // Loading State
            if state.is_loading {
                <div class="text-center">
                    <div class="spinner-border" role="status">
                        <span class="visually-hidden">{"Loading..."}</span>
                    </div>
                </div>
            } else if state.history.is_empty() {
                <div class="text-center py-5">
                    <h5>{"No questions asked yet"}</h5>
                    <p class="text-muted">{"Go to AI Search to start asking questions about your bookmarks."}</p>
                </div>
            } else {
                <div>
                    <div class="mb-3">
                        <small class="text-muted">{format!("Showing {} recent questions", state.history.len())}</small>
                    </div>

                    <div class="accordion" id="historyAccordion">
                        {
                            state.history.iter().enumerate().map(|(index, session)| {
                                render_session(session, index)
                            }).collect::<Html>()
                        }
                    </div>
                </div>
            }
        </div>
    }
}

fn render_session(session: &RagSession, index: usize) -> Html {
    let local_time: DateTime<Local> = session.created_at.into();
    let formatted_time = local_time.format("%Y-%m-%d %H:%M").to_string();
    let item_id = format!("item-{}", index);
    let collapse_id = format!("collapse-{}", index);
    let collapse_target = format!("#collapse-{}", index);
    let is_first = index == 0;

    html! {
        <div class="accordion-item">
            <h2 class="accordion-header" id={item_id.clone()}>
                <button
                    class={if is_first { "accordion-button" } else { "accordion-button collapsed" }}
                    type="button"
                    data-bs-toggle="collapse"
                    data-bs-target={collapse_target}
                    aria-expanded={if is_first { "true" } else { "false" }}
                    aria-controls={collapse_id.clone()}>
                    <div class="w-100 d-flex justify-content-between align-items-start me-3">
                        <div class="flex-grow-1">
                            <strong>{&session.question}</strong>
                            if !session.relevant_chunks.is_empty() {
                                <span class="badge bg-secondary ms-2">{session.relevant_chunks.len()}{" sources"}</span>
                            }
                        </div>
                        <small class="text-muted">{formatted_time}</small>
                    </div>
                </button>
            </h2>
            <div
                id={collapse_id}
                class={if is_first { "accordion-collapse collapse show" } else { "accordion-collapse collapse" }}
                aria-labelledby={item_id}
                data-bs-parent="#historyAccordion">
                <div class="accordion-body">
                    if let Some(ref answer) = session.answer {
                        <div class="border-start border-primary border-3 ps-3">
                            <MarkdownRender content={answer.clone()} />
                        </div>
                    } else {
                        <div class="d-flex align-items-center text-muted">
                            <div class="spinner-border spinner-border-sm me-2" role="status"></div>
                            <em>{"This question is still being processed..."}</em>
                        </div>
                    }
                </div>
            </div>
        </div>
    }
}

impl Reducible for RagHistoryState {
    type Action = RagHistoryMessage;

    fn reduce(self: std::rc::Rc<Self>, action: Self::Action) -> std::rc::Rc<Self> {
        let mut state = (*self).clone();

        match action {
            RagHistoryMessage::SetUserSession(user_session) => {
                state.user_session = user_session;
            }
            RagHistoryMessage::LoadHistory => {
                state.is_loading = true;
                state.error_message = None;
            }
            RagHistoryMessage::HistoryLoaded(result) => {
                state.is_loading = false;
                match result {
                    Ok(sessions) => {
                        state.history = sessions;
                        state.error_message = None;
                    }
                    Err(error) => {
                        state.error_message = Some(error);
                    }
                }
            }
            RagHistoryMessage::ClearError => {
                state.error_message = None;
            }
        }

        std::rc::Rc::new(state)
    }
}
