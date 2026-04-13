use shared::SearchResultItem;
use yew::prelude::*;

use crate::components::atoms::safe_html::BlockquoteHtml;
use crate::router::{self, AppRoute};

#[derive(Debug, Clone, PartialEq, Properties)]
pub struct Props {
    pub results: Vec<SearchResultItem>,
    pub on_item_selected: Callback<String>,
}

fn article(callback: Callback<String>, item: SearchResultItem) -> Html {
    let bookmark_id = item.bookmark.bookmark_id.clone();
    let href = router::href(&AppRoute::Bookmark {
        bookmark_id: bookmark_id.clone(),
    });
    let tags = item
        .bookmark
        .tags
        .unwrap_or_default()
        .into_iter()
        .map(|tag| html! { <span key={tag.clone()} class="badge bg-primary me-1">{tag}</span> })
        .collect::<Vec<_>>();

    let search_match = match item.search_match.clone() {
        Some(html) => html! { <BlockquoteHtml html={html} /> },
        None => html! { <></>},
    };
    let on_click = Callback::from(move |event: MouseEvent| {
        if router::should_handle_spa_navigation(&event) {
            event.prevent_default();
            callback.emit(bookmark_id.clone());
        }
    });
    let summary = if let Some(summary) = &item.bookmark.summary {
        html! {
            <p><em>{summary}</em></p>
        }
    } else {
        html! { <></> }
    };

    html! {
        <div class="card mb-3">
            <div class="card-body">
                <h5 class="card-title">{item.bookmark.title.clone()}</h5>
                <p class="card-text">{search_match}</p>
                <div>{tags}</div>
                <small class="text-muted">{"Created at:"} {item.bookmark.created_at}</small>
                <small><em>{summary}</em></small>
                <a href={href} onclick={on_click} class="btn btn-link mt-2 d-block">{"Read more..."}</a>
            </div>
        </div>
    }
}

#[function_component(SearchResult)]
pub fn search_result(props: &Props) -> Html {
    let results = props.results.clone();
    html! {
        <main>
            {
                results.into_iter().map(|bookmark| {
                    let key = bookmark.bookmark.url.clone();
                    html! {
                        <div key={key}>
                            {article(props.on_item_selected.clone(), bookmark)}
                        </div>
                    }
                }).collect::<Html>()
            }
        </main>
    }
}
