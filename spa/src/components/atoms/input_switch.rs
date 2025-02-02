use wasm_bindgen::JsCast;
use web_sys::{EventTarget, HtmlInputElement};
use yew::prelude::*;

#[derive(PartialEq, Properties, Default)]
pub struct Props {
    pub name: String,
    pub label: String,
    pub on_change: Callback<bool>,
}

#[function_component(InputSwitch)]
pub fn input_switch(props: &Props) -> Html {
    let callback = props.on_change.clone();
    let on_change = Callback::from(move |event: Event| {
        let target: EventTarget = event.target().expect("Fail to cast to EventTarget");
        let value = target.unchecked_into::<HtmlInputElement>().checked();
        callback.emit(value);
    });
    html! {
        <fieldset>
            <label for={props.name.clone()}>
                <input type="checkbox" role="switch" name={props.name.clone()} onchange={on_change} /> {props.label.clone()}
            </label>
        </fieldset>
    }
}
