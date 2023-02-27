use crate::{
    api::auth_api,
    components::composite::login_form::{LoginForm, LoginFormData},
    router::Route,
    user_session::UserSession,
};
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use yew_router::prelude::*;
use yewdux::prelude::*;

#[function_component(Login)]
pub fn login() -> Html {
    let (_, dispatch) = use_store::<UserSession>();
    let history = use_navigator().unwrap();

    let on_login_event = {
        Callback::from(move |event: LoginFormData| {
            let history = history.clone();
            let dispatch = dispatch.clone();
            spawn_local(async move {
                match auth_api::login(event.email.clone(), event.password.clone()).await {
                    Ok(login_response) => {
                        log::info!("User login successful, response={:?}", &login_response);
                        UserSession::login(dispatch.clone(), login_response);
                        history.push(&Route::Home);
                    }
                    Err(error) => {
                        log::warn!("Login failed, error: {}", error);
                        UserSession::logout(dispatch.clone());
                    }
                }
            });
        })
    };

    html! {
        <main class="container mx-auto flex justify-center align-middle">
            <div class="w-full max-w-xs">
                <LoginForm on_login={on_login_event}/>
            </div>
        </main>
    }
}
