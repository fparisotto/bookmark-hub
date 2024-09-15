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
    let state = use_state(LoginFormData::default);

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

    let on_submit = {
        let state = state.clone();
        let on_login = props.on_login.clone();
        Callback::from(move |event: SubmitEvent| {
            event.prevent_default();
            let data = (*state).clone();
            if !data.email.is_empty() && !data.password.is_empty() {
                on_login.emit(data);
            }
        })
    };

    html! {
        <div class="container mt-5">
            <div class="row justify-content-center">
                <div class="col-md-4">
                    <h2 class="text-center mb-4">{ "Login" }</h2>
                    <form onsubmit={on_submit}>
                        <div class="mb-3">
                            <label for="email" class="form-label">{ "Email address" }</label>
                            <InputText
                                id="email"
                                name="email"
                                placeholder="Enter your e-mail"
                                class={"form-control"}
                                input_type={InputType::Email}
                                on_change={on_change_email} />
                        </div>
                        <div class="mb-3">
                            <label for="password" class="form-label">{ "Password" }</label>
                            <InputText
                                id="password"
                                name="password"
                                placeholder="Enter your password"
                                input_type={InputType::Password}
                                class={"form-control"}
                                on_change={on_change_password} />
                        </div>
                        <div class="d-grid">
                            <input class="btn btn-primary" type="submit" value="Login" />
                        </div>
                    </form>
                </div>
            </div>
        </div>
    }
}
