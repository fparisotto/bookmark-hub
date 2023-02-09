use crate::{
    api::bookmarks_api::NewBookmarkRequest,
    components::atoms::input_text::{InputText, InputType},
};
use yew::prelude::*;

#[derive(Debug, PartialEq, Default, Clone)]
pub struct AddBookmarkData {
    pub url: String,
    pub tags: Vec<String>,
}

impl Into<NewBookmarkRequest> for AddBookmarkData {
    fn into(self) -> NewBookmarkRequest {
        NewBookmarkRequest {
            url: self.url.clone(),
            tags: self.tags.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Properties)]
pub struct Props {
    pub id: String,
    pub on_submit: Callback<AddBookmarkData>,
}

#[function_component(AddBookmarkModal)]
pub fn add_bookmark_modal(props: &Props) -> Html {
    let url_state = use_state(|| String::default());
    let tags_state = use_state(|| String::default());

    let on_change_url = {
        let url_state = url_state.clone();
        Callback::from(move |input_text: String| {
            url_state.set(input_text.clone());
        })
    };

    let on_change_tags = {
        let tags_state = tags_state.clone();
        Callback::from(move |input_text: String| {
            tags_state.set(input_text.clone());
        })
    };

    // TODO fix duplicated event handler code
    let on_form_submit = {
        let url_state = url_state.clone();
        let tags_state = tags_state.clone();
        let on_new_bookmark = props.on_submit.clone();
        Callback::from(move |event: SubmitEvent| {
            event.prevent_default();
            on_new_bookmark.emit(AddBookmarkData {
                url: (*url_state).clone(),
                tags: (*tags_state)
                    .split(",")
                    .map(|tag| tag.trim().to_owned())
                    .collect(),
            });
            url_state.set(String::default());
            tags_state.set(String::default());
        })
    };

    // TODO fix duplicated event handler code
    let on_save_click = {
        let url_state = url_state.clone();
        let tags_state = tags_state.clone();
        let on_new_bookmark = props.on_submit.clone();
        Callback::from(move |_event: MouseEvent| {
            on_new_bookmark.emit(AddBookmarkData {
                url: (*url_state).clone(),
                tags: (*tags_state)
                    .split(",")
                    .map(|tag| tag.trim().to_owned())
                    .collect(),
            });
            url_state.set(String::default());
            tags_state.set(String::default());
        })
    };

    html! {
    <div class="modal" id={props.id.clone()}>
        <div class="modal-box">
            <h3 class="font-bold">{"New bookmark"}</h3>
            <form class="form-control py-4" onsubmit={on_form_submit}>
                <label class="label">
                    <span class="label-text">{ "Url" }</span>
                </label>
                <InputText
                    id="url"
                    name="url"
                    placeholder="url..."
                    input_type={InputType::Url}
                    class={classes!("input", "input-bordered")}
                    on_change={on_change_url} />

                <label class="label">
                    <span class="label-text">{ "Tags" }</span>
                </label>
                <InputText
                    id="tags"
                    name="tags"
                    placeholder="tags (separated by ,)"
                    input_type={InputType::Text}
                    class={classes!("input", "input-bordered")}
                    on_change={on_change_tags} />

            </form>
            <div class="modal-action">
                <a href="#" class="btn btn-primary" onclick={on_save_click}>{ "Save" }</a>
                <a href="#" class="btn">{ "Cancel" }</a>
            </div>
        </div>
    </div>
    }
}
