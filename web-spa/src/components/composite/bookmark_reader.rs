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
    pub on_new_tags: Callback<Vec<String>>,
}

#[function_component(BookmarkReader)]
pub fn bookmark_page(props: &Props) -> Html {
    let state = use_state_eq(|| props.bookmark.tags.clone().unwrap_or_default());

    let tags_as_string = (*state).clone().join(", ");

    let on_tag_change: Callback<String> = {
        let state = state.clone();
        Callback::from(move |text: String| {
            let new_tags: Vec<String> = text.split(',').map(|t| t.trim().into()).collect();
            state.set(new_tags);
        })
    };

    let on_goback = {
        let callback = props.on_goback.clone();
        Callback::from(move |event: MouseEvent| {
            event.prevent_default();
            callback.emit(());
        })
    };

    let on_save_tags = {
        let callback = props.on_new_tags.clone();
        Callback::from(move |_: MouseEvent| {
            callback.emit((*state).clone());
        })
    };

    html! {
        <div class="container mx-auto p-4">
            <article class="prose">
                <h1>{"Title: "}{ props.bookmark.title.clone() }</h1>
                <h2>{"Created at: "}{ props.bookmark.user_created_at }</h2>
                <h2>{"Tags: "}
                    <InputText
                        id="tags"
                        name="tags"
                        placeholder="tags..."
                        class={classes!("input", "input-bordered")}
                        input_type={InputType::Text}
                        on_change={on_tag_change}
                        value={tags_as_string} />
                    <button class="btn" onclick={on_save_tags}>{"Save"}</button>
                </h2>
                <h3>{"Original URL: "}<a class="link" href={ props.bookmark.url.clone() }>{ props.bookmark.url.clone() }</a></h3>
                <h4>{"ID: "}{ props.bookmark.bookmark_id.clone() }</h4>
                <ArticleHtml html={ props.bookmark.html_content.clone() } />
            </article>
            <a class="link" href="#" onclick={on_goback}>{ "<< go back to home" }</a>
        </div>
    }
}
