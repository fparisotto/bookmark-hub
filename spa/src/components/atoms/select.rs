use wasm_bindgen::JsCast;
use web_sys::{EventTarget, HtmlInputElement};
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct SelectProps {
    #[prop_or_default]
    pub id: String,
    #[prop_or_default]
    pub name: String,
    #[prop_or_default]
    pub class: String,
    pub options: Vec<String>,
    #[prop_or_default]
    pub selected: Option<String>,
    #[prop_or_default]
    pub on_change: Callback<String>,
}

#[function_component(Select)]
pub fn select(props: &SelectProps) -> Html {
    let on_change = {
        let on_change_cb = props.on_change.clone();
        Callback::from(move |event: Event| {
            let target: EventTarget = event.target().expect("Fail to cast to EventTarget");
            let select_element = target.unchecked_into::<HtmlInputElement>();
            let value = select_element.value();
            on_change_cb.emit(value);
        })
    };

    let options_html = props.options.iter().map(|option| {
        let is_selected = Some(option.clone()) == props.selected;
        html! {
            <option value={option.clone()} selected={is_selected}>
                {option}
            </option>
        }
    });

    html! {
        <select
            id={props.id.clone()}
            name={props.name.clone()}
            class={if props.class.is_empty() {
                "form-select form-select-sm".to_string()
            } else {
                props.class.clone()
            }}
            onchange={on_change}>
            { for options_html }
        </select>
    }
}
