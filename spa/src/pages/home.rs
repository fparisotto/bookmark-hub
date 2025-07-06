use shared::{
    Bookmark, BookmarkTaskSearchRequest, BookmarkTaskSearchResponse, SearchRequest,
    SearchResultItem, TagCount, TagFilter,
};
use yew::{platform::spawn_local, prelude::*};

use crate::{
    api::{bookmark_tasks_api, bookmarks_api, search_api},
    components::composite::{
        add_bookmark_modal::{AddBookmarkData, AddBookmarkModal},
        bookmark_reader::BookmarkReader,
        main_search_result::SearchResult,
        navigation_bar::NavigationBar,
        search_bar::{SearchBar, SearchInputSubmit},
        tags_filter::{TagCheckedEvent, TagsFilter},
        tasks_filter::TasksFilter,
        tasks_table::TasksTable,
    },
    user_session::UserSession,
};

#[derive(Clone, PartialEq, Default, Debug)]
pub struct HomeState {
    pub user_session: UserSession,
    pub items: Vec<SearchResultItem>,
    pub tags: Vec<TagCount>,
    pub tags_filter: Vec<String>,
    pub search_input: String,
    pub new_bookmark_url: String,
    pub new_bookmark_tags: Vec<String>,
    pub bookmark_tasks_response: Option<BookmarkTaskSearchResponse>,
    pub page: Page,
}

#[derive(Clone, PartialEq, Default, Debug)]
pub enum Page {
    #[default]
    Search,
    Read {
        bookmark: Bookmark,
    },
    Tasks,
}

impl From<HomeState> for SearchRequest {
    fn from(value: HomeState) -> Self {
        let query: Option<String> = if value.search_input.is_empty() {
            None
        } else {
            Some(value.search_input)
        };
        let tags_filter: Option<TagFilter> = if value.tags_filter.is_empty() {
            None
        } else {
            Some(TagFilter::Or(value.tags_filter))
        };
        SearchRequest {
            query,
            tags_filter,
            limit: Some(20),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Properties)]
pub struct Props {
    pub user_session: UserSession,
}

#[function_component(Home)]
pub fn home(props: &Props) -> Html {
    let token = props.user_session.token.clone();

    let state_handle = use_state(HomeState::default);

    let on_new_bookmark = {
        let token = token.clone();
        Callback::from(move |event: AddBookmarkData| {
            let token = token.clone();
            spawn_local(async move {
                // FIXME: notify user
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
        let state_handle = state_handle.clone();
        let token = token.clone();
        Callback::from(move |event: SearchInputSubmit| {
            let state_handle = state_handle.clone();
            let token = token.clone();
            spawn_local(async move {
                let mut state = (*state_handle).clone();
                state.search_input = event.input.clone();
                match search_api::search(&token, state.clone().into()).await {
                    Ok(result) => {
                        log::info!("result={:?}", result);
                        state.items = result.items;
                        state.tags = result.tags;
                        state_handle.set(state);
                    }
                    Err(error) => {
                        // FIXME: notify user
                        log::warn!("Fail to search bookmarks, error: {}", error);
                    }
                }
            })
        })
    };

    let on_tag_checked = {
        let state_handle = state_handle.clone();
        Callback::from(move |event: TagCheckedEvent| match event {
            TagCheckedEvent::Checked(tag) => {
                if !state_handle.tags_filter.contains(&tag.tag) {
                    let mut state = (*state_handle).clone();
                    state.tags_filter.push(tag.tag);
                    state_handle.set(state);
                }
            }
            TagCheckedEvent::Unchecked(tag) => {
                if let Some(index_of) = state_handle.tags_filter.iter().position(|e| e == &tag.tag)
                {
                    let mut state = (*state_handle).clone();
                    state.tags_filter.remove(index_of);
                    state_handle.set(state);
                }
            }
        })
    };

    let on_item_selected = {
        let state_handle = state_handle.clone();
        let token = token.clone();
        Callback::from(move |event: SearchResultItem| {
            let state_handle = state_handle.clone();
            let token = token.clone();
            spawn_local(async move {
                match bookmarks_api::get_by_id(&token, &event.bookmark.bookmark_id).await {
                    Ok(Some(bookmark)) => {
                        let mut state = (*state_handle).clone();
                        state.page = Page::Read { bookmark };
                        state_handle.set(state);
                    }
                    Ok(None) => {
                        log::warn!("Weird, bookmark not found in backend, item={:?}", event);
                    }
                    Err(error) => {
                        log::error!("Fail to fetch bookmark, item={:?}, error={error}", event);
                    }
                }
            });
        })
    };

    let on_goback = {
        let state_handle = state_handle.clone();
        Callback::from(move |_| {
            let mut state = (*state_handle).clone();
            state.page = Page::Search;
            state_handle.set(state);
        })
    };

    let on_new_tags = {
        let state_handle = state_handle.clone();
        let token = token.clone();
        Callback::from(move |event: Vec<String>| {
            let token = token.clone();
            let state_handle = state_handle.clone();
            let page = state_handle.page.clone();
            if let Page::Read { bookmark } = page {
                spawn_local(async move {
                    match bookmarks_api::set_tags(&token, &bookmark.bookmark_id, event).await {
                        Ok(bookmark) => {
                            let mut state = (*state_handle).clone();
                            state.page = Page::Read { bookmark };
                            state_handle.set(state);
                        }
                        Err(error) => {
                            log::error!(
                                "Fail to set tags to bookmark={}, error={error}",
                                &bookmark.bookmark_id
                            );
                        }
                    }
                });
            } else {
                let mut state = (*state_handle).clone();
                state.page = Page::Search;
                state_handle.set(state);
                log::warn!(
                    "Invalid state, setting bookmark tags but no bookmark is selected, abort"
                );
            }
        })
    };

    let on_page_change = {
        let state_handle = state_handle.clone();
        Callback::from(move |event: Page| {
            let mut state = (*state_handle).clone();
            state.page = event;
            state_handle.set(state);
        })
    };

    let on_task_filter_submit = {
        let state_handle = state_handle.clone();
        let token = token.clone();
        Callback::from(move |event: BookmarkTaskSearchRequest| {
            let token = token.clone();
            let state_handle = state_handle.clone();
            spawn_local(async move {
                match bookmark_tasks_api::search_tasks(&token, event).await {
                    Ok(response) => {
                        let mut state = (*state_handle).clone();
                        state.bookmark_tasks_response = Some(response);
                        state_handle.set(state);
                    }
                    Err(error) => {
                        // FIXME: notify user
                        log::error!("Fail to search tasks error={error}",);
                        let mut state = (*state_handle).clone();
                        state.bookmark_tasks_response = None;
                        state_handle.set(state);
                    }
                }
            });
        })
    };

    let content = match &state_handle.page {
        Page::Search => {
            html! {
                <>
                    <SearchBar on_submit={on_search_submit} />
                    <TagsFilter tags={state_handle.tags.clone()} on_tag_checked={on_tag_checked} />
                    <SearchResult on_item_selected={on_item_selected} results={state_handle.items.clone()} />
                    <AddBookmarkModal on_submit={on_new_bookmark} />
                </>
            }
        }
        Page::Read { bookmark } => {
            html! {
                <BookmarkReader
                    user_session={props.user_session.to_owned()}
                    bookmark={bookmark.to_owned()}
                    on_goback={on_goback}
                    on_new_tags={on_new_tags} />
            }
        }
        Page::Tasks => {
            html! {
                <>
                    <TasksFilter on_submit={on_task_filter_submit} />
                    <TasksTable response={state_handle.bookmark_tasks_response.clone()} />
                </>
            }
        }
    };

    html! {
        <>
            <NavigationBar username={props.user_session.username.clone()}
                active_page={state_handle.page.clone()}
                on_page_change={on_page_change} />
            <div class="container mt-5">
                {content}
            </div>
        </>
    }
}
