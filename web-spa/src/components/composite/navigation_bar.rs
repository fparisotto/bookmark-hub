use serde::{Deserialize, Serialize};
use yew::prelude::*;
use yewdux::prelude::*;

use crate::api::search_api::SearchType;
use crate::components::atoms::input_checkbox::{InputCheckbox, ItemCheckEvent};
use crate::components::atoms::input_text::{InputText, InputType};
use crate::user_session::UserSession;

#[derive(Debug, PartialEq, Default, Clone, Serialize, Deserialize)]
pub struct SearchInputSubmit {
    pub input: String,
    pub search_type: SearchType,
}

#[derive(PartialEq, Properties)]
pub struct Props {
    pub add_new_bookmark_modal_id: String,
    pub on_submit: Callback<SearchInputSubmit>,
}

#[function_component(NavigationBar)]
pub fn navigation_bar(props: &Props) -> Html {
    let (store, _) = use_store::<UserSession>();

    let state = use_state(SearchInputSubmit::default);

    let email: String = store.email.clone();

    let modal_id = format!("#{}", &props.add_new_bookmark_modal_id);

    let on_input_search_change = {
        let state = state.clone();
        Callback::from(move |text: String| {
            let mut data = (*state).clone();
            data.input = text;
            state.set(data);
        })
    };

    let on_query_switch_change = {
        let state = state.clone();
        Callback::from(move |switch_toggle: ItemCheckEvent| {
            let mut data = (*state).clone();
            match switch_toggle {
                ItemCheckEvent::Checked(_) => data.search_type = SearchType::Query,
                ItemCheckEvent::Unchecked(_) => data.search_type = SearchType::Phrase,
            }
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
        <nav class="col-span-6 navbar">
            <a class="btn btn-ghost uppercase text-lg">{"Bookmarks"}</a>
            <form onsubmit={on_submit} class="flex-1 justify-end gap-2">
                <div class="input-group flex-1 justify-end">
                    <InputText
                        id="search"
                        name="search"
                        placeholder="search..."
                        class={classes!("input", "input-bordered")}
                        input_type={InputType::Search}
                        on_change={on_input_search_change} />
                    <button class="btn">{"Go"}</button>
                </div>
                    <label class="btn btn-outline swap">
                        <InputCheckbox
                            id="toggle-search-type"
                            name="toggle-search-type"
                            value=""
                            on_change={on_query_switch_change} />
                        <div class="swap-on">{ "query" }</div>
                        <div class="swap-off">{ "phrase" }</div>
                    </label>
                <a href={modal_id} class="flex-none btn btn-sm btn-accent">{"Add new"}</a>
            </form>
            <a class="btn btn-ghost normal-case ">{"E-mail: "} {email}</a>
        </nav>
    }
}
