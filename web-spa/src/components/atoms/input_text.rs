use wasm_bindgen::JsCast;
use web_sys::{EventTarget, HtmlInputElement};
use yew::prelude::*;

#[derive(PartialEq, Default, Copy, Clone, Debug)]
pub enum InputType {
    #[default]
    Text,
    Password,
    Email,
    Search,
    Url,
}

impl InputType {
    fn as_type(&self) -> AttrValue {
        match self {
            Self::Text => "text".into(),
            Self::Search => "search".into(),
            Self::Password => "password".into(),
            Self::Email => "email".into(),
            Self::Url => "url".into(),
        }
    }
}

#[derive(PartialEq, Properties, Default, Debug)]
pub struct Props {
    pub id: String,
    pub name: String,
    pub value: Option<String>,
    #[prop_or_default]
    pub class: Classes,
    pub input_type: InputType,
    pub placeholder: String,
    pub on_change: Callback<String>,
}

#[function_component(InputText)]
pub fn input_text(props: &Props) -> Html {
    let callback = props.on_change.clone();
    let on_change = {
        Callback::from(move |event: Event| {
            let target: EventTarget = event.target().expect("Fail to cast to EventTarget");
            let value: String = target.unchecked_into::<HtmlInputElement>().value();
            callback.emit(value.clone());
        })
    };
    html! {
        <input
            id={props.id.clone()}
            name={props.name.clone()}
            value={props.value.clone()}
            type={props.input_type.as_type()}
            placeholder={props.placeholder.clone()}
            class={props.class.clone()}
            onchange={on_change} />
    }
}
