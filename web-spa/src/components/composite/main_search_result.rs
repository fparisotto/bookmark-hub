use yew::prelude::*;

use crate::{api::search_api::SearchResultItem, components::atoms::safe_html::BlockquoteHtml};

#[derive(Debug, Clone, PartialEq, Properties)]
pub struct Props {
    pub results: Vec<SearchResultItem>,
    pub on_item_selected: Callback<SearchResultItem>,
}

fn article(callback: Callback<SearchResultItem>, item: SearchResultItem) -> Html {
    let item_for_event = item.clone();
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
        callback.emit(item_for_event.clone());
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
    let results = props.results.clone();
    html! {
        <main class="container col-span-5 p-4">
            {
                results.into_iter().map(|bookmark| {
                    html! {
                        <>
                            {article(props.on_item_selected.clone(), bookmark)}
                            <div class="divider"></div>
                        </>
                    }
                }).collect::<Html>()
            }
        </main>
    }
}
