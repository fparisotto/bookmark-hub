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
    #[prop_or_default]
    pub value: String,
    #[prop_or_default]
    pub on_clear: Option<Callback<()>>,
    #[prop_or_default]
    pub has_active_filters: bool,
}

#[function_component(SearchBar)]
pub fn search_bar(props: &Props) -> Html {
    let state = use_state(|| SearchInputSubmit {
        input: props.value.clone(),
        ..Default::default()
    });

    {
        let state = state.clone();
        let value = props.value.clone();
        use_effect_with(value.clone(), move |value| {
            let mut data = (*state).clone();
            if data.input != *value {
                data.input = value.clone();
                state.set(data);
            }
        });
    }

    let on_input_search_change = {
        let state = state.clone();
        Callback::from(move |text: String| {
            let mut data = (*state).clone();
            data.input = text;
            state.set(data);
        })
    };

    let current_value = state.input.clone();

    let on_submit = {
        let state = state.clone();
        let on_search = props.on_submit.clone();
        Callback::from(move |event: SubmitEvent| {
            event.prevent_default();
            let data = (*state).clone();
            on_search.emit(data);
        })
    };

    let clear_button = if let Some(on_clear) = &props.on_clear {
        let on_clear = on_clear.clone();
        let has_active_filters = props.has_active_filters;
        html! {
            <button
                type="button"
                class="btn btn-outline-secondary ms-2"
                disabled={!has_active_filters}
                onclick={Callback::from(move |_| on_clear.emit(()))}>
                {"Clear"}
            </button>
        }
    } else {
        html! {}
    };

    html! {
        <div class="d-flex align-items-center">
            <form onsubmit={on_submit} class="d-flex flex-grow-1">
                <InputText
                    id="search"
                    name="search"
                    placeholder="search query"
                    input_type={InputType::Search}
                    class={classes!("form-control", "me-2")}
                    value={Some(current_value)}
                    on_change={on_input_search_change} />
                <input class="btn btn-outline-success" type="submit" value="Search" />
            </form>
            {clear_button}
        </div>
    }
}
