use yew::prelude::*;

use crate::api::tags_api::Tag;
use crate::components::atoms::input_checkbox::{InputCheckbox, ItemCheckEvent};

#[derive(Debug, Clone, PartialEq)]
pub enum TagCheckedEvent {
    Checked(Tag),
    Unchecked(Tag),
}

#[derive(Debug, Clone, PartialEq, Properties)]
pub struct Props {
    pub tags: Vec<Tag>,
    pub on_tag_checked: Callback<TagCheckedEvent>,
}

fn render_tag(on_tag_checked: Callback<TagCheckedEvent>, tag: Tag) -> Html {
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
    let tag_label = format!("{} ({})", &tag.tag, &tag.count);
    html! {
    <li>
        <label class="label cursor-pointer justify-start gap-2">
            <InputCheckbox
                id={tag.tag.clone()}
                name={tag.tag.clone()}
                value={tag.tag.clone()}
                class={classes!("checkbox")}
                on_change={on_change} />
            <span class="label-text">{tag_label}</span>
        </label>
    </li>
    }
}

#[function_component(AsideTags)]
pub fn aside_tags(props: &Props) -> Html {
    let tags = props
        .tags
        .clone()
        .into_iter()
        .map(|tag| render_tag(props.on_tag_checked.clone(), tag))
        .collect::<Html>();
    html! {
    <aside class="col-span-1 p-4">
        <ul>
            {tags}
        </ul>
    </aside>
    }
}
