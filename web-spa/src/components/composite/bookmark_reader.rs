use crate::{
    api::bookmarks_api::Bookmark,
    components::atoms::{
        input_text::{InputText, InputType},
        safe_html::ArticleHtml,
    },
};
use yew::prelude::*;

#[derive(Debug, Clone, PartialEq, Properties)]
pub struct Props {
    pub bookmark: Bookmark,
    pub on_goback: Callback<()>,
}

#[function_component(BookmarkReader)]
pub fn bookmark_page(props: &Props) -> Html {
    let tags = props
        .bookmark
        .tags
        .clone()
        .into_iter()
        .map(|tag| html! { <strong class="badge">{tag}</strong> })
        .collect::<Html>();

    let tags_as_string = props.bookmark.tags.clone().unwrap_or_default().join(", ");

    let on_tag_change: Callback<String> = {
        // let state = state.clone();
        Callback::from(move |text: String| {
            let new_tags: Vec<String> = text.split(',').map(|t| t.trim().into()).collect();
            // let mut inner = (*state).clone();
            // inner.tags = new_tags;
            // state.set(inner);
            log::info!("New tags: {:?}", new_tags);
        })
    };

    let on_goback = {
        let callback = props.on_goback.clone();
        Callback::from(move |event: MouseEvent| {
            event.prevent_default();
            callback.emit(());
        })
    };

    html! {
        <div class="container mx-auto p-4">
            <article class="prose">
                <h1>{"Title: "}{ props.bookmark.title.clone() }</h1>
                <h2>{"Created at: "}{ props.bookmark.user_created_at }</h2>
                <h2>{"Tags: "}{tags}</h2>
                <h2>{"Tags With Input: "}
                    <InputText
                        id="tags"
                        name="tags"
                        placeholder="tags..."
                        input_type={InputType::Text}
                        on_change={on_tag_change}
                        value={tags_as_string} />
                </h2>
                <h3>{"Original URL: "}<a class="link" href={ props.bookmark.url.clone() }>{ props.bookmark.url.clone() }</a></h3>
                <h4>{"ID: "}{ props.bookmark.bookmark_id.clone() }</h4>
                <ArticleHtml html={ props.bookmark.html_content.clone() } />
            </article>
            <a class="link" href="#" onclick={on_goback}>{ "<< go back to home" }</a>
        </div>
    }
}
