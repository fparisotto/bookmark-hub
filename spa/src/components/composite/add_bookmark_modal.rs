use crate::components::atoms::input_text::{InputText, InputType};
use shared::NewBookmarkRequest;
use yew::prelude::*;

#[derive(Debug, PartialEq, Default, Clone)]
pub struct AddBookmarkData {
    pub url: String,
    pub tags: Vec<String>,
}

impl From<AddBookmarkData> for NewBookmarkRequest {
    fn from(value: AddBookmarkData) -> Self {
        NewBookmarkRequest {
            url: value.url,
            tags: value.tags,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Properties)]
pub struct Props {
    pub on_submit: Callback<AddBookmarkData>,
}

#[function_component(AddBookmarkModal)]
pub fn add_bookmark_modal(props: &Props) -> Html {
    let url_state = use_state(String::default);
    let tags_state = use_state(String::default);

    let on_change_url = {
        let url_state = url_state.clone();
        Callback::from(move |input_text: String| {
            url_state.set(input_text);
        })
    };

    let on_change_tags = {
        let tags_state = tags_state.clone();
        Callback::from(move |input_text: String| {
            tags_state.set(input_text);
        })
    };

    let on_form_submit = {
        let url_state = url_state.clone();
        let tags_state = tags_state.clone();
        let on_new_bookmark = props.on_submit.clone();
        Callback::from(move |event: SubmitEvent| {
            event.prevent_default();
            on_new_bookmark.emit(AddBookmarkData {
                url: (*url_state).clone(),
                tags: (*tags_state)
                    .split(',')
                    .map(|tag| tag.trim().to_owned())
                    .collect(),
            });
            url_state.set(String::default());
            tags_state.set(String::default());
        })
    };

    html! {
        <div class="modal fade" id="add-bookmark-modal" tabindex="-1">
            <div class="modal-dialog">
                <div class="modal-content">
                    <div class="modal-header">
                        <h5 class="modal-title" id="bookmarkModalLabel">{"Add a New Bookmark"}</h5>
                        <button type="button" class="btn-close" data-bs-dismiss="modal" aria-label="Close"></button>
                    </div>
                    <div class="modal-body">
                        <form id="bookmark-form" onsubmit={on_form_submit}>
                            <div class="mb-3">
                                <label for="bookmark-link" class="form-label">{"Link"}</label>
                                <InputText
                                    id="url"
                                    name="url"
                                    placeholder="Enter link"
                                    input_type={InputType::Url}
                                    class={classes!("form-control")}
                                    on_change={on_change_url} />
                            </div>
                            <div class="mb-3">
                                <label for="bookmark-tags" class="form-label">{"Tags"}</label>
                                <InputText
                                    id="tags"
                                    name="tags"
                                    placeholder="Enter tags separated by commas"
                                    input_type={InputType::Text}
                                    class={classes!("form-control")}
                                    on_change={on_change_tags} />
                            </div>
                            <input type="submit" class="btn btn-primary" value="Save Bookmark" data-bs-dismiss="modal" data-bs-target="#add-bookmark-modal" />
                        </form>
                    </div>
                </div>
            </div>
        </div>
    }
}
