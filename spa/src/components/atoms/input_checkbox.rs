use wasm_bindgen::JsCast;
use web_sys::{EventTarget, HtmlInputElement};
use yew::prelude::*;

pub enum ItemCheckEvent {
    Checked(String),
    Unchecked(String),
}

#[derive(PartialEq, Properties, Default, Debug)]
pub struct Props {
    pub id: String,
    pub name: String,
    pub value: String,
    pub on_change: Callback<ItemCheckEvent>,
    #[prop_or_default]
    pub class: Classes,
    #[prop_or_default]
    pub checked: bool,
}

#[function_component(InputCheckbox)]
pub fn input_checkbox(props: &Props) -> Html {
    let node_ref = use_node_ref();

    {
        let node_ref = node_ref.clone();
        let checked = props.checked;
        use_effect_with(checked, move |checked| {
            if let Some(input) = node_ref.cast::<HtmlInputElement>() {
                input.set_checked(*checked);
            }
        });
    }

    let callback = props.on_change.clone();
    let on_change = {
        Callback::from(move |event: Event| {
            let target: EventTarget = event.target().expect("Fail to cast to EventTarget");
            let target = target.unchecked_into::<HtmlInputElement>();
            let checked = target.checked();
            let value = target.value();
            if checked {
                callback.emit(ItemCheckEvent::Checked(value));
            } else {
                callback.emit(ItemCheckEvent::Unchecked(value));
            }
        })
    };

    html! {
        <input
            ref={node_ref}
            id={props.id.clone()}
            name={props.name.clone()}
            value={props.value.clone()}
            class={props.class.clone()}
            type="checkbox"
            checked={props.checked}
            onchange={on_change} />
    }
}
