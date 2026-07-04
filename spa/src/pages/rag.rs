use shared::RagQueryRequest;
use yew::platform::spawn_local;
use yew::prelude::*;

use crate::api::rag_api;
use crate::components::composite::rag_query::RagQuery;
use crate::pages::rag_history::RagHistoryTab;
use crate::router::RagTab;
use crate::user_session::UserSession;

#[derive(Clone, PartialEq, Default, Debug)]
pub struct RagState {
    pub current_question: String,
    pub current_response: Option<shared::RagQueryResponse>,
    pub is_loading: bool,
    pub error_message: Option<String>,
}

#[derive(Clone, PartialEq, Debug)]
pub enum RagMessage {
    SetQuestion(String),
    SubmitQuery,
    QueryComplete(Result<shared::RagQueryResponse, String>),
    ClearError,
}

#[derive(Properties, PartialEq)]
pub struct RagPageProps {
    pub user_session: UserSession,
    pub tab: Option<RagTab>,
    pub on_tab_change: Callback<Option<RagTab>>,
}

#[function_component(RagPage)]
pub fn rag_page(props: &RagPageProps) -> Html {
    let active_tab = props.tab.unwrap_or(RagTab::Search);

    let render_tab_link = |label: &'static str, tab: RagTab, active: RagTab| {
        let href = crate::router::href(&crate::router::AppRoute::RAG { tab: Some(tab) });
        let classes = if active == tab {
            classes!("nav-link", "active")
        } else {
            classes!("nav-link")
        };
        let on_tab_change = props.on_tab_change.clone();
        let onclick = Callback::from(move |event: MouseEvent| {
            if crate::router::should_handle_spa_navigation(&event) {
                event.prevent_default();
                on_tab_change.emit(Some(tab));
            }
        });

        html! {
            <li class="nav-item" role="presentation">
                <a href={href} onclick={onclick} class={classes}>{label}</a>
            </li>
        }
    };

    html! {
        <div>
            <div class="mb-4">
                <h1>{"RAG"}</h1>
            </div>

            <ul class="nav nav-tabs mb-4">
                {render_tab_link("Search", RagTab::Search, active_tab)}
                {render_tab_link("History", RagTab::History, active_tab)}
            </ul>

            {
                match active_tab {
                    RagTab::Search => html! { <RagSearchTab user_session={props.user_session.clone()} /> },
                    RagTab::History => html! { <RagHistoryTab user_session={props.user_session.clone()} /> },
                }
            }
        </div>
    }
}

#[derive(Properties, PartialEq)]
pub struct RagSearchTabProps {
    pub user_session: UserSession,
}

#[function_component(RagSearchTab)]
pub fn rag_search_tab(props: &RagSearchTabProps) -> Html {
    let state = use_reducer(RagState::default);

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
                    max_context_tokens: None,
                    hybrid_search: None,
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
        <div>
            if let Some(error) = &state.error_message {
                <div class="alert alert-danger alert-dismissible" role="alert">
                    {error}
                    <button type="button" class="btn-close" onclick={on_clear_error}></button>
                </div>
            }

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

impl Reducible for RagState {
    type Action = RagMessage;

    fn reduce(self: std::rc::Rc<Self>, action: Self::Action) -> std::rc::Rc<Self> {
        let mut state = (*self).clone();

        match action {
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
