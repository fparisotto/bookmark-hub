use yew::{platform::spawn_local, prelude::*};

use crate::{
    api::{
        bookmarks_api::{self, Bookmark},
        search_api::{self, SearchRequest, SearchResultItem, TagFilter},
        tags_api::Tag,
    },
    components::composite::{
        add_bookmark_modal::{AddBookmarkData, AddBookmarkModal},
        bookmark_reader::BookmarkReader,
        main_search_result::MainSearchResult,
        navigation_bar::NavigationBar,
        search_bar::{SearchBar, SearchInputSubmit},
        tags_filter::{TagCheckedEvent, TagsFilter},
    },
    user_session::UserSession,
};

#[derive(Clone, PartialEq, Default, Debug)]
pub struct HomeState {
    pub user_session: UserSession,
    pub bookmarks: Vec<SearchResultItem>,
    pub tags: Vec<Tag>,
    pub tags_filter: Vec<String>,
    pub search_input: String,
    pub new_bookmark_url: String,
    pub new_bookmark_tags: Vec<String>,
    pub bookmark_read: Option<Bookmark>,
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

    let state = use_state(HomeState::default);

    let on_new_bookmark = {
        let token = token.clone();
        Callback::from(move |event: AddBookmarkData| {
            let token = token.clone();
            spawn_local(async move {
                // FIXME notify user
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
        let state = state.clone();
        let token = token.clone();
        Callback::from(move |event: SearchInputSubmit| {
            let state = state.clone();
            let token = token.clone();
            spawn_local(async move {
                let mut home = (*state).clone();
                home.search_input = event.input.clone();
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

    let on_item_selected = {
        let state = state.clone();
        let token = token.clone();
        Callback::from(move |event: SearchResultItem| {
            let state = state.clone();
            let token = token.clone();
            spawn_local(async move {
                match bookmarks_api::get_by_id(&token, &event.bookmark_id).await {
                    Ok(Some(bookmark)) => {
                        let mut home = (*state).clone();
                        home.bookmark_read = Some(bookmark);
                        state.set(home);
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
        let state = state.clone();
        Callback::from(move |_| {
            let mut home = (*state).clone();
            home.bookmark_read = None;
            state.set(home);
        })
    };

    let on_new_tags = {
        let state = state.clone();
        let token = token.clone();
        Callback::from(move |event: Vec<String>| {
            let token = token.clone();
            let state = state.clone();
            let bookmark_id = state.bookmark_read.clone().expect("not none").bookmark_id;
            spawn_local(async move {
                match bookmarks_api::set_tags(&token, &bookmark_id, event).await {
                    Ok(bookmark) => {
                        let mut home = (*state).clone();
                        home.bookmark_read = Some(bookmark);
                        state.set(home);
                    }
                    Err(error) => {
                        log::error!("Fail to set tags to bookmark={bookmark_id}, error={error}",);
                    }
                }
            });
        })
    };

    let bookmark_read = state.bookmark_read.clone();

    let content = if let Some(bookmark) = bookmark_read {
        html! {
            <BookmarkReader
                user_session={props.user_session.clone()}
                bookmark={bookmark}
                on_goback={on_goback}
                on_new_tags={on_new_tags} />
        }
    } else {
        html! {
            <>
                <SearchBar on_submit={on_search_submit} />
                <TagsFilter tags={state.tags.clone()} on_tag_checked={on_tag_checked} />
                <MainSearchResult on_item_selected={on_item_selected} results={state.bookmarks.clone()} />
                <AddBookmarkModal on_submit={on_new_bookmark} />
            </>
        }
    };

    html! {
        <>
            <NavigationBar email={props.user_session.email.clone()} />
            <div class="container mt-5">
                {content}
            </div>
        </>
    }
}
