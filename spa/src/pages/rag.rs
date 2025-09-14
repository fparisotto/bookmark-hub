use shared::{RagQueryRequest, RagQueryResponse};
use yew::platform::spawn_local;
use yew::prelude::*;

use crate::api::rag_api;
use crate::components::composite::rag_query::RagQuery;
use crate::user_session::UserSession;

#[derive(Clone, PartialEq, Default, Debug)]
pub struct RagState {
    pub user_session: UserSession,
    pub current_question: String,
    pub current_response: Option<RagQueryResponse>,
    pub is_loading: bool,
    pub error_message: Option<String>,
}

#[derive(Clone, PartialEq, Debug)]
pub enum RagMessage {
    SetUserSession(UserSession),
    SetQuestion(String),
    SubmitQuery,
    QueryComplete(Result<RagQueryResponse, String>),
    ClearError,
}

#[function_component(RagPage)]
pub fn rag_page(props: &RagPageProps) -> Html {
    let state = use_reducer(|| RagState {
        user_session: props.user_session.clone(),
        ..Default::default()
    });

    let on_question_change = {
        let state = state.clone();
        Callback::from(move |question: String| {
            state.dispatch(RagMessage::SetQuestion(question));
        })
    };

    let on_submit = {
        let state = state.clone();
        let user_session = props.user_session.clone();
        Callback::from(move |_| {
            let state = state.clone();
            let user_session = user_session.clone();
            let question = state.current_question.clone();

            state.dispatch(RagMessage::SubmitQuery);

            spawn_local(async move {
                let request = RagQueryRequest {
                    question,
                    max_chunks: Some(10),
                    similarity_threshold: Some(0.3),
                };

                let result = rag_api::query_rag(&user_session, &request).await;
                let action = match result {
                    Ok(response) => RagMessage::QueryComplete(Ok(response)),
                    Err(err) => RagMessage::QueryComplete(Err(format!("Query failed: {}", err))),
                };

                state.dispatch(action);
            });
        })
    };

    let on_clear_error = {
        let state = state.clone();
        Callback::from(move |_| {
            state.dispatch(RagMessage::ClearError);
        })
    };

    html! {
        <div class="container mt-5">
            <div class="mb-4">
                <h1>{"AI Search & Question Answering"}</h1>
                <p class="text-muted">{"Ask questions about your bookmarks and get AI-powered answers with source citations."}</p>
            </div>

            // Error Display
            if let Some(error) = &state.error_message {
                <div class="alert alert-danger alert-dismissible" role="alert">
                    {error}
                    <button type="button" class="btn-close" onclick={on_clear_error}></button>
                </div>
            }

            // Main RAG Query Component
            <RagQuery
                question={state.current_question.clone()}
                response={state.current_response.clone()}
                is_loading={state.is_loading}
                on_question_change={on_question_change}
                on_submit={on_submit}
            />
        </div>
    }
}

#[derive(Properties, PartialEq)]
pub struct RagPageProps {
    pub user_session: UserSession,
}

impl Reducible for RagState {
    type Action = RagMessage;

    fn reduce(self: std::rc::Rc<Self>, action: Self::Action) -> std::rc::Rc<Self> {
        let mut state = (*self).clone();

        match action {
            RagMessage::SetUserSession(user_session) => {
                state.user_session = user_session;
            }
            RagMessage::SetQuestion(question) => {
                state.current_question = question;
                state.error_message = None;
            }
            RagMessage::SubmitQuery => {
                if state.current_question.trim().is_empty() {
                    state.error_message = Some("Please enter a question".to_string());
                    return std::rc::Rc::new(state);
                }

                state.is_loading = true;
                state.error_message = None;
                state.current_response = None;
            }
            RagMessage::QueryComplete(result) => {
                state.is_loading = false;
                match result {
                    Ok(response) => {
                        state.current_response = Some(response);
                        state.error_message = None;
                    }
                    Err(error) => {
                        state.error_message = Some(error);
                        state.current_response = None;
                    }
                }
            }
            RagMessage::ClearError => {
                state.error_message = None;
            }
        }

        std::rc::Rc::new(state)
    }
}
