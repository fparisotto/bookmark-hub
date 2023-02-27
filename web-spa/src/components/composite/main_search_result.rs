use yew::prelude::*;
use yew_router::prelude::{use_navigator, Navigator};

use crate::{
    api::search_api::SearchResultItem, components::atoms::safe_html::BlockquoteHtml, router::Route,
};

#[derive(Debug, Clone, PartialEq, Properties)]
pub struct Props {
    pub results: Vec<SearchResultItem>,
}

fn article(history: Navigator, item: SearchResultItem) -> Html {
    let tags = item
        .tags
        .unwrap_or_default()
        .into_iter()
        .map(|tag| html! { <strong class="badge">{tag}</strong> })
        .collect::<Html>();
    let search_match = match item.search_match.clone() {
        Some(html) => html! { <BlockquoteHtml html={html} /> },
        None => html! { <></>},
    };
    let on_click = Callback::from(move |_| {
        history.push(&Route::Bookmark {
            id: item.bookmark_id.clone(),
        })
    });
    html! {
    <article class="prose">
        <header>
            <h2 class="text-lg">{item.title.clone()}</h2>
            <span class="block">{"Tags:"} {tags} </span>
            <span class="block">{"Created at:"}<strong>{item.created_at}</strong></span>
        </header>
        {search_match}
        <a class="link" onclick={on_click}>{"Read..."}</a>
    </article>
    }
}

#[function_component(MainSearchResult)]
pub fn main_search_result(props: &Props) -> Html {
    let history = use_navigator().expect("navigator");
    let results = props.results.clone();
    html! {
    <main class="container col-span-5 p-4">
        {
            results.into_iter().map(|bookmark| {
                html! {
                    <>
                        {article(history.clone(), bookmark)}
                        <div class="divider"></div>
                    </>
                }
            }).collect::<Html>()
        }
    </main>
    }
}
