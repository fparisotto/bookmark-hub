use yew::prelude::*;
use yew_router::prelude::use_navigator;
use yewdux::prelude::*;

use crate::{
    api::{
        bookmarks_api,
        search_api::{self, SearchRequest, SearchType, TagFilterType, SearchResultItem},
        tags_api::Tag,
    },
    components::composite::{
        add_bookmark_modal::{AddBookmarkData, AddBookmarkModal},
        aside_tags::{AsideTags, TagCheckedEvent},
        main_search_result::MainSearchResult,
        navigation_bar::{NavigationBar, SearchInputSubmit},
    },
    router::Route,
    user_session::UserSession,
};

#[derive(Clone, PartialEq, Default, Debug)]
pub struct HomeState {
    pub bookmarks: Vec<SearchResultItem>,
    pub tags: Vec<Tag>,
    pub tags_filter: Vec<String>,
    pub search_input: String,
    pub search_type: SearchType,
    pub new_bookmark_url: String,
    pub new_bookmark_tags: Vec<String>,
}

impl Into<SearchRequest> for HomeState {
    fn into(self) -> SearchRequest {
        let mut query: Option<String> = None;
        let mut phrase: Option<String> = None;
        let mut tags: Option<Vec<String>> = None;
        match self.search_type {
            SearchType::Query if !self.search_input.is_empty() => {
                query = Some(self.search_input);
            }
            SearchType::Phrase if !self.search_input.is_empty() => {
                phrase = Some(self.search_input);
            }
            _ => (),
        }
        if !self.tags_filter.is_empty() {
            tags = Some(self.tags_filter.clone());
        }
        SearchRequest {
            query,
            phrase,
            tags,
            tags_filter_type: Some(TagFilterType::Or), // FIXME missing UI for this
            limit: None,
        }
    }
}

#[function_component(Home)]
pub fn home() -> Html {
    let (store, _) = use_store::<UserSession>();
    let token = store.token.clone();

    // FIXME
    let history = use_navigator().expect("navigator");
    if !store.logged() {
        history.push(&Route::Login);
        log::info!("User not logged");
    }

    let state = use_state(|| HomeState::default());

    let on_new_bookmark = {
        let token = token.clone();
        Callback::from(move |event: AddBookmarkData| {
            let token = token.clone();
            wasm_bindgen_futures::spawn_local(async move {
                // FIXME notify the user about the outcome
                match bookmarks_api::add_bookmark(&token, event.into()).await {
                    Ok(result) => log::info!(
                        "New bookmark added, response={}",
                        serde_json::to_string(&result).unwrap()
                    ),
                    Err(error) => log::warn!("Add bookmark failed, error: {}", error),
                }
            })
        })
    };

    let on_search_submit = {
        let token = token.clone();
        let state = state.clone();
        Callback::from(move |event: SearchInputSubmit| {
            let state = state.clone();
            let token = token.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let mut home = (*state).clone();
                home.search_input = event.input.clone();
                home.search_type = event.search_type.clone();
                match search_api::search(&token, home.clone().into()).await {
                    Ok(result) => {
                        log::info!("result={:?}", result);
                        home.bookmarks = result.bookmarks;
                        home.tags = result.tags;
                        state.set(home);
                    }
                    Err(error) => {
                        // FIXME notify user
                        log::warn!("Fail to search bookmarks, error: {}", error);
                    }
                }
            })
        })
    };

    let on_tag_checked = {
        let state = state.clone();
        Callback::from(move |event: TagCheckedEvent| match event {
            TagCheckedEvent::Checked(tag) => {
                if !state.tags_filter.contains(&tag.tag) {
                    let mut home = (*state).clone();
                    home.tags_filter.push(tag.tag);
                    state.set(home);
                }
            }
            TagCheckedEvent::Unchecked(tag) => {
                if let Some(index_of) = state.tags_filter.iter().position(|e| e == &tag.tag) {
                    let mut home = (*state).clone();
                    home.tags_filter.remove(index_of);
                    state.set(home);
                }
            }
        })
    };

    html! {
        <>
            <div class="container mx-auto grid grid-cols-6">
                <NavigationBar add_new_bookmark_modal_id="add-new-bookmark-modal" on_submit={on_search_submit} />
                <AsideTags tags={state.tags.clone()} on_tag_checked={on_tag_checked} />
                <MainSearchResult results={state.bookmarks.clone()} />
            </div>
            <AddBookmarkModal id="add-new-bookmark-modal" on_submit={on_new_bookmark} />
        </>
    }
}

