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
        pagination_controls::PaginationControls,
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
    pub bookmark_tasks_request: BookmarkTaskSearchRequest,
    pub page: Page,
    pub current_search_page: usize,
    pub page_size: usize,
    pub total_results: u64,
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
        let offset = if value.current_search_page > 1 {
            Some(((value.current_search_page - 1) * value.page_size) as i32)
        } else {
            None
        };
        SearchRequest {
            query,
            tags_filter,
            limit: Some(value.page_size as i32),
            offset,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Properties)]
pub struct Props {
    pub user_session: UserSession,
    pub on_logout: Callback<()>,
}

#[function_component(Home)]
pub fn home(props: &Props) -> Html {
    let token = props.user_session.token.clone();

    let state_handle = use_state(|| HomeState {
        page_size: 20,
        current_search_page: 1,
        ..Default::default()
    });

    // Trigger initial search on component mount
    {
        let state_handle = state_handle.clone();
        let token = token.clone();
        use_effect_with((), move |_| {
            let state_handle = state_handle.clone();
            let token = token.clone();
            spawn_local(async move {
                let state = (*state_handle).clone();
                // Create an empty search request (no query, no filters)
                let search_request: SearchRequest = state.into();
                match search_api::search(&token, search_request).await {
                    Ok(result) => {
                        log::info!("Initial search loaded, items count={}", result.items.len());
                        let mut state = (*state_handle).clone();
                        state.items = result.items;
                        state.tags = result.tags;
                        state.total_results = result.total;
                        state_handle.set(state);
                    }
                    Err(error) => {
                        log::warn!("Failed to load initial bookmarks, error: {error}");
                    }
                }
            });
        });
    }

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
                    Err(error) => log::warn!("Add bookmark failed, error: {error}"),
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
                state.current_search_page = 1; // Reset to first page on new search
                match search_api::search(&token, state.clone().into()).await {
                    Ok(result) => {
                        log::info!("result={result:?}");
                        state.items = result.items;
                        state.tags = result.tags;
                        state.total_results = result.total;
                        state_handle.set(state);
                    }
                    Err(error) => {
                        // FIXME: notify user
                        log::warn!("Fail to search bookmarks, error: {error}");
                    }
                }
            })
        })
    };

    let on_tag_checked = {
        let state_handle = state_handle.clone();
        let token = token.clone();
        Callback::from(move |event: TagCheckedEvent| {
            let state_handle = state_handle.clone();
            let token = token.clone();
            let mut state = (*state_handle).clone();
            match event {
                TagCheckedEvent::Checked(tag) => {
                    if !state.tags_filter.contains(&tag.tag) {
                        state.tags_filter.push(tag.tag);
                        state.current_search_page = 1; // Reset to first page on filter change
                    }
                }
                TagCheckedEvent::Unchecked(tag) => {
                    if let Some(index_of) = state.tags_filter.iter().position(|e| e == &tag.tag) {
                        state.tags_filter.remove(index_of);
                        state.current_search_page = 1; // Reset to first page on filter change
                    }
                }
            }
            let state_clone = state.clone();
            state_handle.set(state);
            // Trigger search with new filters
            spawn_local(async move {
                match search_api::search(&token, state_clone.into()).await {
                    Ok(result) => {
                        let mut state = (*state_handle).clone();
                        state.items = result.items;
                        state.tags = result.tags;
                        state.total_results = result.total;
                        state_handle.set(state);
                    }
                    Err(error) => {
                        log::warn!("Fail to search bookmarks with new filter, error: {error}");
                    }
                }
            });
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
                        log::warn!("Weird, bookmark not found in backend, item={event:?}");
                    }
                    Err(error) => {
                        log::error!("Fail to fetch bookmark, item={event:?}, error={error}");
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

    // Pagination callbacks for search page
    let on_search_previous_page = {
        let state_handle = state_handle.clone();
        let token = token.clone();
        Callback::from(move |_| {
            let state_handle = state_handle.clone();
            let token = token.clone();
            let current_page = state_handle.current_search_page;
            if current_page > 1 {
                spawn_local(async move {
                    let mut state = (*state_handle).clone();
                    state.current_search_page = current_page - 1;
                    match search_api::search(&token, state.clone().into()).await {
                        Ok(result) => {
                            state.items = result.items;
                            state.tags = result.tags;
                            state.total_results = result.total;
                            state_handle.set(state);
                        }
                        Err(error) => {
                            log::error!("Fail to load previous page, error={error}");
                        }
                    }
                });
            }
        })
    };

    let on_search_next_page = {
        let state_handle = state_handle.clone();
        let token = token.clone();
        Callback::from(move |_| {
            let state_handle = state_handle.clone();
            let token = token.clone();
            let state = (*state_handle).clone();
            let total_pages = (state.total_results as usize).div_ceil(state.page_size);
            if state.current_search_page < total_pages {
                spawn_local(async move {
                    let mut state = (*state_handle).clone();
                    state.current_search_page += 1;
                    match search_api::search(&token, state.clone().into()).await {
                        Ok(result) => {
                            state.items = result.items;
                            state.tags = result.tags;
                            state.total_results = result.total;
                            state_handle.set(state);
                        }
                        Err(error) => {
                            log::error!("Fail to load next page, error={error}");
                        }
                    }
                });
            }
        })
    };

    // Pagination tracking state - keep track of all page cursors
    let page_cursors_handle = use_state(|| vec![None::<uuid::Uuid>]);
    let current_page_handle = use_state(|| 1usize);

    let on_task_filter_submit = {
        let state_handle = state_handle.clone();
        let token = token.clone();
        let page_cursors_handle = page_cursors_handle.clone();
        let current_page_handle = current_page_handle.clone();
        Callback::from(move |event: BookmarkTaskSearchRequest| {
            let token = token.clone();
            let state_handle = state_handle.clone();
            let page_cursors_handle = page_cursors_handle.clone();
            let current_page_handle = current_page_handle.clone();

            // Reset pagination state when new filter is applied
            page_cursors_handle.set(vec![None]);
            current_page_handle.set(1);

            spawn_local(async move {
                let mut state = (*state_handle).clone();
                // Store the request for pagination
                state.bookmark_tasks_request = event.clone();
                match bookmark_tasks_api::search_tasks(&token, event).await {
                    Ok(response) => {
                        state.bookmark_tasks_response = Some(response);
                        state_handle.set(state);
                    }
                    Err(error) => {
                        // FIXME: notify user
                        log::error!("Fail to search tasks error={error}",);
                        state.bookmark_tasks_response = None;
                        state_handle.set(state);
                    }
                }
            });
        })
    };

    let on_previous_page = {
        let state_handle = state_handle.clone();
        let token = token.clone();
        let page_cursors_handle = page_cursors_handle.clone();
        let current_page_handle = current_page_handle.clone();
        Callback::from(move |_| {
            let token = token.clone();
            let state_handle = state_handle.clone();
            let page_cursors_handle = page_cursors_handle.clone();
            let current_page_handle = current_page_handle.clone();
            let current_page = *current_page_handle;
            if current_page > 1 {
                let new_page = current_page - 1;
                current_page_handle.set(new_page);
                let cursor = page_cursors_handle[new_page - 1];
                spawn_local(async move {
                    let state = (*state_handle).clone();
                    let mut request = state.bookmark_tasks_request.clone();
                    request.last_task_id = cursor;
                    match bookmark_tasks_api::search_tasks(&token, request).await {
                        Ok(response) => {
                            let mut state = (*state_handle).clone();
                            state.bookmark_tasks_response = Some(response);
                            state_handle.set(state);
                        }
                        Err(error) => {
                            log::error!("Fail to load previous page, error={error}");
                        }
                    }
                });
            }
        })
    };

    let on_next_page = {
        let state_handle = state_handle.clone();
        let token = token.clone();
        let page_cursors_handle = page_cursors_handle.clone();
        let current_page_handle = current_page_handle.clone();
        Callback::from(move |_| {
            let token = token.clone();
            let state_handle = state_handle.clone();
            let page_cursors_handle = page_cursors_handle.clone();
            let current_page_handle = current_page_handle.clone();
            let state = (*state_handle).clone();
            if let Some(response) = &state.bookmark_tasks_response {
                if response.has_more && !response.tasks.is_empty() {
                    let last_task = response.tasks.last().unwrap();
                    let cursor = Some(last_task.task_id);

                    // Add new cursor to the list if needed
                    let mut cursors = (*page_cursors_handle).clone();
                    let current_page = *current_page_handle;
                    if cursors.len() == current_page {
                        cursors.push(cursor);
                        page_cursors_handle.set(cursors);
                    }

                    current_page_handle.set(current_page + 1);

                    spawn_local(async move {
                        let mut request = state.bookmark_tasks_request.clone();
                        request.last_task_id = cursor;
                        match bookmark_tasks_api::search_tasks(&token, request).await {
                            Ok(response) => {
                                let mut state = (*state_handle).clone();
                                state.bookmark_tasks_response = Some(response);
                                state_handle.set(state);
                            }
                            Err(error) => {
                                log::error!("Fail to load next page, error={error}");
                            }
                        }
                    });
                }
            }
        })
    };

    let content = match &state_handle.page {
        Page::Search => {
            let has_more = state_handle.current_search_page * state_handle.page_size
                < state_handle.total_results as usize;
            html! {
                <>
                    <SearchBar on_submit={on_search_submit} />
                    <TagsFilter tags={state_handle.tags.clone()} on_tag_checked={on_tag_checked} />
                    <SearchResult on_item_selected={on_item_selected} results={state_handle.items.clone()} />
                    <div class="mt-3">
                        <PaginationControls
                            has_more={has_more}
                            on_previous={on_search_previous_page}
                            on_next={on_search_next_page}
                            current_page={state_handle.current_search_page}
                            page_size={state_handle.page_size}
                            current_count={state_handle.items.len()} />
                    </div>
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
            let pagination_controls = if let Some(response) = &state_handle.bookmark_tasks_response
            {
                let page_size =
                    state_handle.bookmark_tasks_request.page_size.unwrap_or(25) as usize;
                html! {
                    <PaginationControls
                        has_more={response.has_more}
                        on_previous={on_previous_page}
                        on_next={on_next_page}
                        current_page={*current_page_handle}
                        page_size={page_size}
                        current_count={response.tasks.len()} />
                }
            } else {
                html! {}
            };

            html! {
                <>
                    <TasksFilter on_submit={on_task_filter_submit} />
                    <TasksTable response={state_handle.bookmark_tasks_response.clone()} />
                    <div class="mt-3">
                        {pagination_controls}
                    </div>
                </>
            }
        }
    };

    html! {
        <>
            <NavigationBar username={props.user_session.username.clone()}
                active_page={state_handle.page.clone()}
                on_page_change={on_page_change}
                on_logout={props.on_logout.clone()} />
            <div class="container mt-5">
                {content}
            </div>
        </>
    }
}
