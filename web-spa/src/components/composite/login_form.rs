use crate::components::atoms::input_text::{InputText, InputType};
use yew::prelude::*;

#[derive(Debug, PartialEq, Default, Clone)]
pub struct LoginFormData {
    pub email: String,
    pub password: String,
}

#[derive(PartialEq, Properties)]
pub struct Props {
    pub on_login: Callback<LoginFormData>,
}

#[function_component(LoginForm)]
pub fn login_form(props: &Props) -> Html {
    let state = use_state(|| LoginFormData::default());

    let on_change_email = {
        let state = state.clone();
        Callback::from(move |input_text: String| {
            let mut data: LoginFormData = (*state).clone();
            data.email = input_text;
            state.set(data);
        })
    };

    let on_change_password = {
        let state = state.clone();
        Callback::from(move |input_text: String| {
            let mut data = (*state).clone();
            data.password = input_text;
            state.set(data);
        })
    };

    // TODO remove duplicated code for event handling
    let on_submit = {
        let state = state.clone();
        let on_login = props.on_login.clone();
        Callback::from(move |event: SubmitEvent| {
            event.prevent_default();
            let data = (*state).clone();
            if !data.email.is_empty() && !data.password.is_empty() {
                on_login.emit(data.clone());
            }
        })
    };

    let on_click = {
        let state = state.clone();
        let on_login = props.on_login.clone();
        Callback::from(move |_: MouseEvent| {
            let data = (*state).clone();
            if !data.email.is_empty() && !data.password.is_empty() {
                on_login.emit(data.clone());
            }
        })
    };

    html! {
        <form class="px-8 pt-6 pb-8 mb-4" onsubmit={on_submit}>
            <label class="block text-sm font-bold mb-2" for="username"> {"E-mail"}
                <InputText
                    id="email"
                    name="email"
                    placeholder="e-mail"
                    class={classes!("input", "input-bordered", "w-full", "max-w-x")}
                    input_type={InputType::Email}
                    on_change={on_change_email} />
            </label>
            <label class="block text-sm font-bold mb-2" for="username"> {"Password"}
                <InputText
                    id="password"
                    name="password"
                    placeholder="password"
                    input_type={InputType::Password}
                    class={classes!("input", "input-bordered", "w-full", "max-w-x")}
                    on_change={on_change_password} />
            </label>
            <button class="btn" type="button" onclick={on_click}>{"Sign In"}</button>
        </form>
    }
}
