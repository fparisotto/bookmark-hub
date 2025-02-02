use serde::{Deserialize, Serialize};
use shared::SearchType;
use yew::prelude::*;

use crate::components::atoms::input_text::{InputText, InputType};

#[derive(Debug, PartialEq, Default, Clone, Serialize, Deserialize)]
pub struct SearchInputSubmit {
    pub input: String,
    pub search_type: SearchType,
}

#[derive(PartialEq, Properties)]
pub struct Props {
    pub on_submit: Callback<SearchInputSubmit>,
}

#[function_component(SearchBar)]
pub fn search_bar(props: &Props) -> Html {
    let state = use_state(SearchInputSubmit::default);

    let on_input_search_change = {
        let state = state.clone();
        Callback::from(move |text: String| {
            let mut data = (*state).clone();
            data.input = text;
            state.set(data);
        })
    };

    let on_submit = {
        let on_search = props.on_submit.clone();
        Callback::from(move |event: SubmitEvent| {
            event.prevent_default();
            let data = (*state).clone();
            on_search.emit(data);
        })
    };

    html! {
        <>
            <div class="d-flex justify-content-between align-items-center mb-4">
                <form onsubmit={on_submit} class="d-flex" style="width: 100%;">
                    <InputText
                        id="search"
                        name="search"
                        placeholder="search query"
                        input_type={InputType::Search}
                        class={classes!("form-control", "me-2")}
                        on_change={on_input_search_change} />
                    <input class="btn btn-outline-success" type="submit" value="Search" />
                </form>
            </div>
        </>
    }
}
