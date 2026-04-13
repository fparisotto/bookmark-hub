use url::form_urlencoded;
use wasm_bindgen::JsValue;
use web_sys::History;
use yew::MouseEvent;

const HISTORY_STATE_DIRECT: &str = "bookmark-hub:direct";
const HISTORY_STATE_PUSHED: &str = "bookmark-hub:pushed";

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum AppRoute {
    Search(SearchRouteState),
    Bookmark { bookmark_id: String },
    Tasks,
    RAG,
    RagHistory,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RouteKind {
    Search,
    Bookmark,
    Tasks,
    RAG,
    RagHistory,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SearchRouteState {
    pub query: String,
    pub tags: Vec<String>,
    pub page: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HistoryEntryState {
    Direct,
    Pushed,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ParsedRoute {
    pub route: AppRoute,
    pub needs_canonical_url: bool,
}

impl Default for SearchRouteState {
    fn default() -> Self {
        Self {
            query: String::new(),
            tags: Vec::new(),
            page: 1,
        }
    }
}

impl SearchRouteState {
    pub fn new(query: impl Into<String>, tags: Vec<String>, page: usize) -> Self {
        let mut normalized_tags = tags
            .into_iter()
            .map(|tag| tag.trim().to_string())
            .filter(|tag| !tag.is_empty())
            .collect::<Vec<_>>();
        normalized_tags.sort();
        normalized_tags.dedup();

        Self {
            query: query.into().trim().to_string(),
            tags: normalized_tags,
            page: page.max(1),
        }
    }
}

impl AppRoute {
    pub fn kind(&self) -> RouteKind {
        match self {
            Self::Search(_) => RouteKind::Search,
            Self::Bookmark { .. } => RouteKind::Bookmark,
            Self::Tasks => RouteKind::Tasks,
            Self::RAG => RouteKind::RAG,
            Self::RagHistory => RouteKind::RagHistory,
        }
    }
}

pub fn href(route: &AppRoute) -> String {
    match route {
        AppRoute::Search(search) => {
            let mut serializer = form_urlencoded::Serializer::new(String::new());
            if !search.query.is_empty() {
                serializer.append_pair("q", &search.query);
            }
            for tag in &search.tags {
                serializer.append_pair("tag", tag);
            }
            if search.page > 1 {
                serializer.append_pair("page", &search.page.to_string());
            }

            let query = serializer.finish();
            if query.is_empty() {
                "/".to_string()
            } else {
                format!("/?{query}")
            }
        }
        AppRoute::Bookmark { bookmark_id } => format!("/bookmarks/{bookmark_id}"),
        AppRoute::Tasks => "/tasks".to_string(),
        AppRoute::RAG => "/rag".to_string(),
        AppRoute::RagHistory => "/rag/history".to_string(),
    }
}

pub fn parse_current_route() -> ParsedRoute {
    let window = web_sys::window().expect("window should be available");
    let location = window.location();
    let pathname = location
        .pathname()
        .expect("location pathname should be available");
    let search = location
        .search()
        .expect("location search should be available");
    parse_path_and_search(&pathname, &search)
}

pub fn sync_current_route(default_state: HistoryEntryState) -> AppRoute {
    let parsed = parse_current_route();
    let current_state = current_entry_state().unwrap_or(default_state);
    if parsed.needs_canonical_url || current_entry_state().is_none() {
        let _ = replace_with_state(&parsed.route, current_state);
    }
    parsed.route
}

pub fn push(route: &AppRoute) -> Result<(), JsValue> {
    update_history(route, HistoryEntryState::Pushed, false)
}

pub fn replace_as_direct(route: &AppRoute) -> Result<(), JsValue> {
    replace_with_state(route, HistoryEntryState::Direct)
}

pub fn replace_preserving_entry(route: &AppRoute) -> Result<(), JsValue> {
    replace_with_state(
        route,
        current_entry_state().unwrap_or(HistoryEntryState::Direct),
    )
}

pub fn current_entry_was_pushed() -> bool {
    current_entry_state() == Some(HistoryEntryState::Pushed)
}

pub fn back() -> Result<(), JsValue> {
    history()?.back()
}

pub fn should_handle_spa_navigation(event: &MouseEvent) -> bool {
    event.button() == 0
        && !event.ctrl_key()
        && !event.meta_key()
        && !event.shift_key()
        && !event.alt_key()
        && !event.default_prevented()
}

fn replace_with_state(route: &AppRoute, entry_state: HistoryEntryState) -> Result<(), JsValue> {
    update_history(route, entry_state, true)
}

fn update_history(
    route: &AppRoute,
    entry_state: HistoryEntryState,
    replace: bool,
) -> Result<(), JsValue> {
    let url = href(route);
    let state = JsValue::from_str(match entry_state {
        HistoryEntryState::Direct => HISTORY_STATE_DIRECT,
        HistoryEntryState::Pushed => HISTORY_STATE_PUSHED,
    });

    if replace {
        history()?.replace_state_with_url(&state, "", Some(&url))
    } else {
        history()?.push_state_with_url(&state, "", Some(&url))
    }
}

fn history() -> Result<History, JsValue> {
    web_sys::window()
        .expect("window should be available")
        .history()
}

fn current_entry_state() -> Option<HistoryEntryState> {
    let state = history().ok()?.state().ok()?;
    match state.as_string().as_deref() {
        Some(HISTORY_STATE_DIRECT) => Some(HistoryEntryState::Direct),
        Some(HISTORY_STATE_PUSHED) => Some(HistoryEntryState::Pushed),
        _ => None,
    }
}

fn parse_path_and_search(pathname: &str, search: &str) -> ParsedRoute {
    let path = normalize_path(pathname);
    let route = match path.as_str() {
        "/" => AppRoute::Search(parse_search_state(search)),
        "/tasks" => AppRoute::Tasks,
        "/rag" => AppRoute::RAG,
        "/rag/history" => AppRoute::RagHistory,
        _ if path.starts_with("/bookmarks/") => {
            let bookmark_id = path.trim_start_matches("/bookmarks/").to_string();
            if bookmark_id.is_empty() || bookmark_id.contains('/') {
                AppRoute::Search(SearchRouteState::default())
            } else {
                AppRoute::Bookmark { bookmark_id }
            }
        }
        _ => AppRoute::Search(SearchRouteState::default()),
    };

    let canonical_url = href(&route);
    let raw_url = if search.is_empty() {
        path.clone()
    } else {
        format!("{path}{search}")
    };

    ParsedRoute {
        route,
        needs_canonical_url: canonical_url != raw_url,
    }
}

fn parse_search_state(search: &str) -> SearchRouteState {
    let mut query = String::new();
    let mut tags = Vec::new();
    let mut page = 1usize;

    for (key, value) in form_urlencoded::parse(search.trim_start_matches('?').as_bytes()) {
        match key.as_ref() {
            "q" if query.is_empty() => query = value.trim().to_string(),
            "tag" => tags.push(value.into_owned()),
            "page" if page == 1 => {
                if let Ok(parsed_page) = value.parse::<usize>() {
                    if parsed_page > 0 {
                        page = parsed_page;
                    }
                }
            }
            _ => {}
        }
    }

    SearchRouteState::new(query, tags, page)
}

fn normalize_path(pathname: &str) -> String {
    let trimmed = pathname.trim();
    if trimmed.is_empty() || trimmed == "/" {
        return "/".to_string();
    }

    let normalized = trimmed.trim_end_matches('/');
    if normalized.is_empty() {
        "/".to_string()
    } else if normalized.starts_with('/') {
        normalized.to_string()
    } else {
        format!("/{normalized}")
    }
}

#[cfg(test)]
mod tests {
    use super::{href, parse_path_and_search, AppRoute, SearchRouteState};

    #[test]
    fn parses_default_search_route() {
        let parsed = parse_path_and_search("/", "");
        assert_eq!(parsed.route, AppRoute::Search(SearchRouteState::default()));
        assert!(!parsed.needs_canonical_url);
    }

    #[test]
    fn parses_query_tags_and_page() {
        let parsed = parse_path_and_search("/", "?tag=rust&tag=yew&q=hello&page=2");
        assert_eq!(
            parsed.route,
            AppRoute::Search(SearchRouteState::new(
                "hello",
                vec!["rust".to_string(), "yew".to_string()],
                2
            ))
        );
        assert!(parsed.needs_canonical_url);
    }

    #[test]
    fn canonicalizes_invalid_page_and_duplicate_tags() {
        let parsed = parse_path_and_search("/", "?page=0&tag=yew&tag=yew");
        assert_eq!(
            parsed.route,
            AppRoute::Search(SearchRouteState::new("", vec!["yew".to_string()], 1))
        );
        assert!(parsed.needs_canonical_url);
    }

    #[test]
    fn parses_bookmark_route() {
        let parsed = parse_path_and_search("/bookmarks/abc123", "");
        assert_eq!(
            parsed.route,
            AppRoute::Bookmark {
                bookmark_id: "abc123".to_string()
            }
        );
        assert!(!parsed.needs_canonical_url);
    }

    #[test]
    fn unknown_paths_fall_back_to_search() {
        let parsed = parse_path_and_search("/missing/path", "");
        assert_eq!(parsed.route, AppRoute::Search(SearchRouteState::default()));
        assert!(parsed.needs_canonical_url);
    }

    #[test]
    fn href_omits_default_search_values() {
        let route = AppRoute::Search(SearchRouteState::new("", vec![], 1));
        assert_eq!(href(&route), "/");
    }
}
