use shared::TagCount;
use yew::prelude::*;

use crate::components::atoms::input_checkbox::{InputCheckbox, ItemCheckEvent};

#[derive(Debug, Clone, PartialEq)]
pub enum TagCheckedEvent {
    Checked(TagCount),
    Unchecked(TagCount),
}

#[derive(Debug, Clone, PartialEq, Properties)]
pub struct Props {
    pub tags: Vec<TagCount>,
    pub on_tag_checked: Callback<TagCheckedEvent>,
}

fn render_tag(on_tag_checked: Callback<TagCheckedEvent>, tag: TagCount) -> Html {
    let on_change = {
        let tag = tag.clone();
        Callback::from(move |event: ItemCheckEvent| match event {
            ItemCheckEvent::Checked(_) => {
                on_tag_checked.emit(TagCheckedEvent::Checked(tag.clone()))
            }
            ItemCheckEvent::Unchecked(_) => {
                on_tag_checked.emit(TagCheckedEvent::Unchecked(tag.clone()))
            }
        })
    };

    html! {
        <div class="form-check">
            <InputCheckbox
                id={tag.tag.clone()}
                name={tag.tag.clone()}
                value={tag.tag.clone()}
                class={classes!("form-check-input")}
                on_change={on_change} />
            <label class="form-check-label d-flex justify-content-between align-items-center" for={tag.tag.clone()}>
                <span class="text-truncate">{tag.tag.clone()}</span>
                <span class="badge bg-secondary ms-2">{tag.count}</span>
            </label>
        </div>
    }
}

#[function_component(TagsFilter)]
pub fn tags_filter(props: &Props) -> Html {
    let tags = props
        .tags
        .clone()
        .into_iter()
        .map(|tag| render_tag(props.on_tag_checked.clone(), tag))
        .collect::<Html>();

    html! {
        <div class="tags-filter-panel bg-body-secondary border-end p-3 h-100">
            <h6 class="mb-3 text-muted fw-bold">{"Filter by Tags"}</h6>
            <div class="d-flex flex-column gap-2">
                {tags}
            </div>
        </div>
    }
}
