use shared::{
    Bookmark, BookmarkTaskSearchRequest, BookmarkTaskSearchResponse, SearchRequest, TagCount,
    TagFilter,
};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use yew::platform::spawn_local;
use yew::prelude::*;

use crate::api::{bookmark_tasks_api, bookmarks_api, search_api};
use crate::components::composite::add_bookmark_modal::{AddBookmarkData, AddBookmarkModal};
use crate::components::composite::bookmark_reader::BookmarkReader;
use crate::components::composite::main_search_result::SearchResult;
use crate::components::composite::navigation_bar::NavigationBar;
use crate::components::composite::pagination_controls::PaginationControls;
use crate::components::composite::search_bar::{SearchBar, SearchInputSubmit};
use crate::components::composite::tags_filter::{TagCheckedEvent, TagsFilter};
use crate::components::composite::tasks_filter::TasksFilter;
use crate::components::composite::tasks_table::TasksTable;
use crate::router::{self, AppRoute, HistoryEntryState, RouteKind, SearchRouteState};
use crate::user_session::UserSession;

#[derive(Clone, PartialEq, Default, Debug)]
pub struct HomeState {
    pub user_session: UserSession,
    pub items: Vec<shared::SearchResultItem>,
    pub tags: Vec<TagCount>,
    pub tags_filter: Vec<String>,
    pub search_input: String,
    pub new_bookmark_url: String,
    pub new_bookmark_tags: Vec<String>,
    pub bookmark_tasks_response: Option<BookmarkTaskSearchResponse>,
    pub bookmark_tasks_request: BookmarkTaskSearchRequest,
    pub current_search_page: usize,
    pub page_size: usize,
    pub total_results: u64,
}

#[derive(Clone, PartialEq, Default, Debug)]
pub enum BookmarkDetailState {
    #[default]
    None,
    Loading {
        bookmark_id: String,
    },
    Ready(Bookmark),
    NotFound {
        bookmark_id: String,
    },
    Error {
        bookmark_id: String,
        message: String,
    },
}

impl HomeState {
    fn current_search_route(&self) -> AppRoute {
        AppRoute::Search(SearchRouteState::new(
            self.search_input.clone(),
            self.tags_filter.clone(),
            self.current_search_page,
        ))
    }
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

    let state_handle = use_state_eq(|| HomeState {
        page_size: 20,
        current_search_page: 1,
        ..Default::default()
    });
    let bookmark_detail_handle = use_state_eq(BookmarkDetailState::default);
    let route_handle = use_state_eq(|| router::sync_current_route(HistoryEntryState::Direct));

    {
        let route_handle = route_handle.clone();
        use_effect_with((), move |_| {
            let current_route = router::sync_current_route(HistoryEntryState::Direct);
            route_handle.set(current_route);

            let window = web_sys::window().expect("window should be available");
            let listener_route_handle = route_handle.clone();
            let callback =
                Closure::<dyn FnMut(web_sys::PopStateEvent)>::wrap(Box::new(move |_| {
                    let current_route = router::sync_current_route(HistoryEntryState::Direct);
                    listener_route_handle.set(current_route);
                }));

            let _ = window
                .add_event_listener_with_callback("popstate", callback.as_ref().unchecked_ref());

            move || {
                let _ = window.remove_event_listener_with_callback(
                    "popstate",
                    callback.as_ref().unchecked_ref(),
                );
            }
        });
    }

    {
        let state_handle = state_handle.clone();
        let bookmark_detail_handle = bookmark_detail_handle.clone();
        let route_handle = route_handle.clone();
        let token = token.clone();
        use_effect_with((*route_handle).clone(), move |route| match route.clone() {
            AppRoute::Search(search_route) => {
                bookmark_detail_handle.set(BookmarkDetailState::None);

                let mut state = (*state_handle).clone();
                state.search_input = search_route.query.clone();
                state.tags_filter = search_route.tags.clone();
                state.current_search_page = search_route.page;
                state_handle.set(state.clone());

                let state_handle = state_handle.clone();
                let route_handle = route_handle.clone();
                let token = token.clone();
                spawn_local(async move {
                    match search_api::search(&token, state.clone().into()).await {
                        Ok(result) => {
                            if *route_handle != AppRoute::Search(search_route.clone()) {
                                return;
                            }

                            let mut new_state = (*state_handle).clone();
                            new_state.items = result.items;
                            new_state.tags = result.tags;
                            new_state.total_results = result.total;
                            state_handle.set(new_state);
                        }
                        Err(error) => {
                            log::warn!("Failed to load search route, error: {error}");
                        }
                    }
                });
            }
            AppRoute::Bookmark { bookmark_id } => {
                bookmark_detail_handle.set(BookmarkDetailState::Loading {
                    bookmark_id: bookmark_id.clone(),
                });

                let bookmark_detail_handle = bookmark_detail_handle.clone();
                let route_handle = route_handle.clone();
                let token = token.clone();
                spawn_local(async move {
                    match bookmarks_api::get_by_id(&token, &bookmark_id).await {
                        Ok(Some(bookmark)) => {
                            if *route_handle
                                != (AppRoute::Bookmark {
                                    bookmark_id: bookmark_id.clone(),
                                })
                            {
                                return;
                            }
                            bookmark_detail_handle.set(BookmarkDetailState::Ready(bookmark));
                        }
                        Ok(None) => {
                            if *route_handle
                                == (AppRoute::Bookmark {
                                    bookmark_id: bookmark_id.clone(),
                                })
                            {
                                bookmark_detail_handle
                                    .set(BookmarkDetailState::NotFound { bookmark_id });
                            }
                        }
                        Err(error) => {
                            log::error!(
                                "Failed to fetch bookmark for route, bookmark_id={}, error={error}",
                                &bookmark_id
                            );
                            if *route_handle
                                == (AppRoute::Bookmark {
                                    bookmark_id: bookmark_id.clone(),
                                })
                            {
                                bookmark_detail_handle.set(BookmarkDetailState::Error {
                                    bookmark_id,
                                    message: "Failed to load bookmark.".to_string(),
                                });
                            }
                        }
                    }
                });
            }
            _ => {
                bookmark_detail_handle.set(BookmarkDetailState::None);
            }
        });
    }

    let navigate_with_push = {
        let route_handle = route_handle.clone();
        Callback::from(move |route: AppRoute| match router::push(&route) {
            Ok(_) => route_handle.set(route),
            Err(error) => log::error!("Failed to push route to browser history: {error:?}"),
        })
    };

    let on_new_bookmark = {
        let token = token.clone();
        Callback::from(move |event: AddBookmarkData| {
            let token = token.clone();
            spawn_local(async move {
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
        let navigate_with_push = navigate_with_push.clone();
        Callback::from(move |event: SearchInputSubmit| {
            let route = AppRoute::Search(SearchRouteState::new(
                event.input,
                state_handle.tags_filter.clone(),
                1,
            ));
            navigate_with_push.emit(route);
        })
    };

    let on_tag_checked = {
        let state_handle = state_handle.clone();
        let navigate_with_push = navigate_with_push.clone();
        Callback::from(move |event: TagCheckedEvent| {
            let mut tags = state_handle.tags_filter.clone();
            match event {
                TagCheckedEvent::Checked(tag) => {
                    if !tags.contains(&tag.tag) {
                        tags.push(tag.tag);
                    }
                }
                TagCheckedEvent::Unchecked(tag) => {
                    if let Some(index_of) = tags.iter().position(|candidate| candidate == &tag.tag)
                    {
                        tags.remove(index_of);
                    }
                }
            }

            let route = AppRoute::Search(SearchRouteState::new(
                state_handle.search_input.clone(),
                tags,
                1,
            ));
            navigate_with_push.emit(route);
        })
    };

    let on_clear_filters = {
        let navigate_with_push = navigate_with_push.clone();
        Callback::from(move |_: ()| {
            navigate_with_push.emit(AppRoute::Search(SearchRouteState::default()));
        })
    };

    let on_item_selected = {
        let navigate_with_push = navigate_with_push.clone();
        Callback::from(move |bookmark_id: String| {
            navigate_with_push.emit(AppRoute::Bookmark { bookmark_id });
        })
    };

    let on_goback = {
        let route_handle = route_handle.clone();
        let state_handle = state_handle.clone();
        Callback::from(move |_| {
            if router::current_entry_was_pushed() {
                if let Err(error) = router::back() {
                    log::error!("Failed to navigate browser history back: {error:?}");
                    let fallback = state_handle.current_search_route();
                    if let Err(replace_error) = router::replace_as_direct(&fallback) {
                        log::error!(
                            "Failed to replace route after back fallback: {replace_error:?}"
                        );
                    } else {
                        route_handle.set(fallback);
                    }
                }
            } else {
                let fallback = state_handle.current_search_route();
                if let Err(error) = router::replace_as_direct(&fallback) {
                    log::error!("Failed to replace direct bookmark route: {error:?}");
                } else {
                    route_handle.set(fallback);
                }
            }
        })
    };

    let on_new_tags = {
        let bookmark_detail_handle = bookmark_detail_handle.clone();
        let token = token.clone();
        Callback::from(move |event: Vec<String>| {
            let token = token.clone();
            let bookmark_detail_handle = bookmark_detail_handle.clone();
            let detail_state = (*bookmark_detail_handle).clone();
            if let BookmarkDetailState::Ready(bookmark) = detail_state {
                spawn_local(async move {
                    match bookmarks_api::set_tags(&token, &bookmark.bookmark_id, event).await {
                        Ok(bookmark) => {
                            bookmark_detail_handle.set(BookmarkDetailState::Ready(bookmark));
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
                log::warn!("Invalid state, setting tags without an active bookmark detail");
            }
        })
    };

    let on_delete = {
        let bookmark_detail_handle = bookmark_detail_handle.clone();
        let route_handle = route_handle.clone();
        let state_handle = state_handle.clone();
        let token = token.clone();
        Callback::from(move |_: ()| {
            let token = token.clone();
            let bookmark_detail_handle = bookmark_detail_handle.clone();
            let route_handle = route_handle.clone();
            let state_handle = state_handle.clone();
            let detail_state = (*bookmark_detail_handle).clone();
            if let BookmarkDetailState::Ready(bookmark) = detail_state {
                spawn_local(async move {
                    match bookmarks_api::delete_bookmark(&token, &bookmark.bookmark_id).await {
                        Ok(true) => {
                            log::info!("Bookmark deleted, id={}", &bookmark.bookmark_id);
                            if router::current_entry_was_pushed() {
                                if let Err(error) = router::back() {
                                    log::error!(
                                        "Failed to go back after delete, bookmark_id={}, error={error:?}",
                                        &bookmark.bookmark_id
                                    );
                                }
                            } else {
                                let fallback = state_handle.current_search_route();
                                if let Err(error) = router::replace_as_direct(&fallback) {
                                    log::error!(
                                        "Failed to replace route after delete, bookmark_id={}, error={error:?}",
                                        &bookmark.bookmark_id
                                    );
                                } else {
                                    route_handle.set(fallback);
                                }
                            }
                        }
                        Ok(false) => {
                            log::warn!(
                                "Bookmark already missing during delete, id={}",
                                &bookmark.bookmark_id
                            );
                        }
                        Err(error) => {
                            log::error!(
                                "Failed to delete bookmark={}, error={error}",
                                &bookmark.bookmark_id
                            );
                        }
                    }
                });
            }
        })
    };

    let on_page_change = {
        let state_handle = state_handle.clone();
        let navigate_with_push = navigate_with_push.clone();
        Callback::from(move |event: RouteKind| {
            let route = match event {
                RouteKind::Search => state_handle.current_search_route(),
                RouteKind::Tasks => AppRoute::Tasks,
                RouteKind::RAG => AppRoute::RAG,
                RouteKind::RagHistory => AppRoute::RagHistory,
                RouteKind::Bookmark => return,
            };
            navigate_with_push.emit(route);
        })
    };

    let on_search_previous_page = {
        let state_handle = state_handle.clone();
        let navigate_with_push = navigate_with_push.clone();
        Callback::from(move |_| {
            let current_page = state_handle.current_search_page;
            if current_page > 1 {
                navigate_with_push.emit(AppRoute::Search(SearchRouteState::new(
                    state_handle.search_input.clone(),
                    state_handle.tags_filter.clone(),
                    current_page - 1,
                )));
            }
        })
    };

    let on_search_next_page = {
        let state_handle = state_handle.clone();
        let navigate_with_push = navigate_with_push.clone();
        Callback::from(move |_| {
            let total_pages =
                (state_handle.total_results as usize).div_ceil(state_handle.page_size);
            if state_handle.current_search_page < total_pages {
                navigate_with_push.emit(AppRoute::Search(SearchRouteState::new(
                    state_handle.search_input.clone(),
                    state_handle.tags_filter.clone(),
                    state_handle.current_search_page + 1,
                )));
            }
        })
    };

    let page_cursors_handle = use_state_eq(|| vec![None::<uuid::Uuid>]);
    let current_page_handle = use_state_eq(|| 1usize);

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

            page_cursors_handle.set(vec![None]);
            current_page_handle.set(1);

            spawn_local(async move {
                let mut state = (*state_handle).clone();
                state.bookmark_tasks_request = event.clone();
                match bookmark_tasks_api::search_tasks(&token, event).await {
                    Ok(response) => {
                        state.bookmark_tasks_response = Some(response);
                        state_handle.set(state);
                    }
                    Err(error) => {
                        log::error!("Fail to search tasks error={error}");
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

    let content = match &*route_handle {
        AppRoute::Search(_) => {
            let has_more = state_handle.current_search_page * state_handle.page_size
                < state_handle.total_results as usize;
            html! {
                <div class="row" style="min-height: calc(100vh - 120px);">
                    <div class="col-12 col-md-4 col-lg-3 mb-3 mb-md-0 d-flex">
                        <TagsFilter
                            tags={state_handle.tags.clone()}
                            selected_tags={state_handle.tags_filter.clone()}
                            on_tag_checked={on_tag_checked} />
                    </div>
                    <div class="col-12 col-md-8 col-lg-9">
                        <SearchBar
                            value={state_handle.search_input.clone()}
                            on_submit={on_search_submit}
                            on_clear={Some(on_clear_filters.clone())}
                            has_active_filters={!state_handle.tags_filter.is_empty() || !state_handle.search_input.is_empty()} />
                        <div class="mt-3">
                            <SearchResult on_item_selected={on_item_selected} results={state_handle.items.clone()} />
                        </div>
                        <div class="mt-3">
                            <PaginationControls
                                has_more={has_more}
                                on_previous={on_search_previous_page}
                                on_next={on_search_next_page}
                                current_page={state_handle.current_search_page}
                                page_size={state_handle.page_size}
                                current_count={state_handle.items.len()} />
                        </div>
                    </div>
                </div>
            }
        }
        AppRoute::Bookmark { .. } => match &*bookmark_detail_handle {
            BookmarkDetailState::Loading { .. } | BookmarkDetailState::None => {
                html! {
                    <div class="text-center py-5">
                        <div class="spinner-border" role="status">
                            <span class="visually-hidden">{"Loading..."}</span>
                        </div>
                    </div>
                }
            }
            BookmarkDetailState::Ready(bookmark) => {
                html! {
                    <BookmarkReader
                        key={bookmark.bookmark_id.clone()}
                        user_session={props.user_session.to_owned()}
                        bookmark={bookmark.to_owned()}
                        on_goback={on_goback}
                        on_new_tags={on_new_tags}
                        on_delete={on_delete.clone()} />
                }
            }
            BookmarkDetailState::NotFound { .. } => {
                html! {
                    <div class="alert alert-warning" role="alert">
                        <h4 class="alert-heading">{"Bookmark not found"}</h4>
                        <p>{"The requested bookmark no longer exists or is not available."}</p>
                        <hr />
                        <button class="btn btn-outline-secondary" onclick={
                            let on_goback = on_goback.clone();
                            Callback::from(move |_| on_goback.emit(()))
                        }>
                            {"Back to search"}
                        </button>
                    </div>
                }
            }
            BookmarkDetailState::Error { message, .. } => {
                html! {
                    <div class="alert alert-danger" role="alert">
                        <h4 class="alert-heading">{"Unable to load bookmark"}</h4>
                        <p>{message.clone()}</p>
                        <hr />
                        <button class="btn btn-outline-secondary" onclick={
                            let on_goback = on_goback.clone();
                            Callback::from(move |_| on_goback.emit(()))
                        }>
                            {"Back to search"}
                        </button>
                    </div>
                }
            }
        },
        AppRoute::Tasks => {
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
        AppRoute::RAG => {
            html! {
                <crate::pages::rag::RagPage user_session={props.user_session.clone()} />
            }
        }
        AppRoute::RagHistory => {
            html! {
                <crate::pages::rag_history::RagHistoryPage user_session={props.user_session.clone()} />
            }
        }
    };

    html! {
        <>
            <NavigationBar username={props.user_session.username.clone()}
                active_page={(*route_handle).kind()}
                on_page_change={on_page_change}
                on_logout={props.on_logout.clone()} />
            <div class="container mt-5">
                {content}
            </div>
            <AddBookmarkModal on_submit={on_new_bookmark} />
        </>
    }
}
