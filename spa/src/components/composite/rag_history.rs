use chrono::{DateTime, Local};
use shared::RagSession;
use yew::prelude::*;

use crate::user_session::UserSession;

#[derive(Properties, PartialEq)]
pub struct RagHistoryProps {
    pub sessions: Vec<RagSession>,
    pub user_session: UserSession,
}

#[function_component(RagHistory)]
pub fn rag_history(props: &RagHistoryProps) -> Html {
    if props.sessions.is_empty() {
        return html! {
            <div class="text-center py-16">
                <div class="text-gray-400 text-6xl mb-6">{"💬"}</div>
                <h3 class="text-xl font-medium text-white mb-3">{"No questions asked yet"}</h3>
                <p class="text-gray-400 text-base">{"Switch to the Ask Question tab to start using AI search."}</p>
            </div>
        };
    }

    html! {
        <div class="space-y-6">
            <div class="text-sm text-gray-400 mb-6">
                {"Showing " }{props.sessions.len()}{" recent questions"}
            </div>

            {
                props.sessions.iter().map(|session| {
                    render_session(session)
                }).collect::<Html>()
            }
        </div>
    }
}

fn render_session(session: &RagSession) -> Html {
    let local_time: DateTime<Local> = session.created_at.into();
    let formatted_time = local_time.format("%Y-%m-%d %H:%M").to_string();

    html! {
        <div class="bg-gray-800 rounded-xl border border-gray-700 p-8 space-y-6 hover:border-gray-600 transition-colors">
            <div class="flex items-start justify-between">
                <div class="flex-1">
                    <h3 class="font-semibold text-white mb-3 flex items-center">
                        <span class="mr-2">{"❓"}</span>
                        {"Question"}
                    </h3>
                    <p class="text-gray-300 leading-relaxed">{&session.question}</p>
                </div>
                <div class="text-sm text-gray-400 whitespace-nowrap ml-6">
                    {formatted_time}
                </div>
            </div>

            if let Some(ref answer) = session.answer {
                <div class="border-t border-gray-700 pt-6">
                    <h4 class="font-semibold text-white mb-3 flex items-center">
                        <span class="mr-2">{"🤖"}</span>
                        {"Answer"}
                    </h4>
                    <div class="text-gray-300 prose prose-invert max-w-none">
                        <p class="whitespace-pre-wrap leading-relaxed">{answer}</p>
                    </div>
                </div>
            } else {
                <div class="text-gray-500 italic border-t border-gray-700 pt-6">
                    <span class="flex items-center">
                        <svg class="animate-spin -ml-1 mr-2 h-4 w-4 text-current" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                            <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                            <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                        </svg>
                        {"This question is still being processed..."}
                    </span>
                </div>
            }

            if !session.relevant_chunks.is_empty() {
                <div class="pt-4 border-t border-gray-700">
                    <span class="text-xs text-gray-400 flex items-center">
                        <span class="mr-2">{"📚"}</span>
                        {session.relevant_chunks.len()}{" relevant chunks found"}
                    </span>
                </div>
            }
        </div>
    }
}
