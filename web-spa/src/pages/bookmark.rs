use crate::{router::Route, user_session::UserSession, api::bookmarks_api, api::bookmarks_api::Bookmark, components::atoms::safe_html::ArticleHtml};
use chrono::{DateTime, Utc};
use yew::prelude::*;
use yew_router::prelude::*;
use yewdux::prelude::*;

#[derive(Clone, PartialEq, Default, Debug)]
pub struct State {
    pub id: String,
    pub title: String,
    pub url: String,
    pub html_content: String,
    pub tags: Vec<String>,
    pub user_created_at: DateTime<Utc>,
}

impl From<Bookmark> for State {
    fn from(value: Bookmark) -> Self {
        State {
            id: value.bookmark_id,
            title: value.title,
            url: value.url,
            html_content: value.html_content,
            tags: value.tags.unwrap_or_default(),
            user_created_at: value.user_created_at,
        }
    }
}


#[derive(Debug, Clone, PartialEq, Properties)]
pub struct Props {
    pub bookmark_id: String,
}

#[function_component(BookmarkPage)]
pub fn bookmark_page(props: &Props) -> Html {
    let (store, _) = use_store::<UserSession>();
    let history = use_navigator().expect("navigator");
    if !store.logged() {
        history.push(&Route::Login);
        log::info!("User not logged");
    }

    let state = use_state_eq(|| State::default());

    let token = store.token.clone();
    let bookmark_id = props.bookmark_id.clone();
    let state_cloned = state.clone();
    use_effect(move || {
        let state_cloned = state_cloned.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let state_cloned = state_cloned.clone();
            match bookmarks_api::get_by_id(&token, &bookmark_id).await {
                Ok(Some(bookmark)) => {
                    state_cloned.set(bookmark.into());
                },
                _ => {
                    history.push(&Route::Home);
                }
            }
        });
        || {}
    });

    let tags = (*state).tags
        .clone()
        .into_iter()
        .map(|tag| html! { <strong class="badge">{tag}</strong> })
        .collect::<Html>();

    html! {
        <div class="container mx-auto p-4">
            <article class="prose">
                <h1>{"Title: "}{ (*state).title.clone() }</h1>
                <h2>{"Created at: "}{ (*state).user_created_at.clone() }</h2>
                <h2>{"Tags: "}{tags}</h2>
                <h3>{"Original URL: "}<a class="link" href={(*state).url.clone()}>{ (*state).url.clone() }</a></h3>
                <h4>{"ID: "}{ (*state).id.clone() }</h4>
                <ArticleHtml html={(*state).html_content.clone()} />
            </article>
            <Link<Route> to={Route::Home}>{ "<< go back to home" }</Link<Route>>
        </div>
    }
}
