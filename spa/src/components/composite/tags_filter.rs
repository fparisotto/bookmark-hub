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
    pub selected_tags: Vec<String>,
    pub on_tag_checked: Callback<TagCheckedEvent>,
}

fn render_tag(on_tag_checked: Callback<TagCheckedEvent>, tag: TagCount, is_checked: bool) -> Html {
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
        <div class="form-check d-flex align-items-center py-1" style="min-height: 1.75rem;">
            <InputCheckbox
                id={tag.tag.clone()}
                name={tag.tag.clone()}
                value={tag.tag.clone()}
                class={classes!("form-check-input", "flex-shrink-0")}
                checked={is_checked}
                on_change={on_change} />
            <label class="form-check-label text-truncate ms-2" for={tag.tag.clone()} title={tag.tag.clone()}>
                {tag.tag.clone()}
            </label>
            <span class="badge bg-secondary ms-auto flex-shrink-0">{tag.count}</span>
        </div>
    }
}

#[function_component(TagsFilter)]
pub fn tags_filter(props: &Props) -> Html {
    let mut sorted_tags = props.tags.clone();
    sorted_tags.sort_by(|a, b| b.count.cmp(&a.count));

    let selected_tags = &props.selected_tags;
    let tags = sorted_tags
        .into_iter()
        .map(|tag| {
            let is_checked = selected_tags.contains(&tag.tag);
            render_tag(props.on_tag_checked.clone(), tag, is_checked)
        })
        .collect::<Html>();

    html! {
        <div class="tags-filter-panel bg-body-secondary border-end p-3 h-100">
            <h6 class="text-muted fw-bold mb-3">{"Filter by Tags"}</h6>
            <div class="d-flex flex-column gap-2" style="max-height: 400px; overflow-y: auto;">
                {tags}
            </div>
        </div>
    }
}
