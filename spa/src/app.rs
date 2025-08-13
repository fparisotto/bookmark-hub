use yew::platform::spawn_local;
use yew::prelude::*;
use yew_hooks::prelude::*;

use crate::{
    api::auth_api,
    components::composite::login_form::{LoginForm, LoginFormData},
    pages::home::Home,
    user_session::UserSession,
};

#[function_component(App)]
pub fn app() -> Html {
    let storage = use_local_storage::<UserSession>("user-session".to_string());
    let logged = use_state(|| false);
    let is_first = use_is_first_mount();

    if is_first {
        let logged = logged.clone();
        let storage = storage.clone();
        match storage.as_ref() {
            Some(user_session) if user_session.logged() => {
                let token = user_session.token.clone();
                spawn_local(async move {
                    match auth_api::get_user_profile(token).await {
                        Ok(_) => {
                            logged.set(true);
                        }
                        Err(error) => {
                            log::warn!(
                                "Fail to fetch user profile, cleaning session. Error={error}"
                            );
                            storage.delete();
                            logged.set(false);
                        }
                    }
                });
            }
            _ => {
                storage.delete();
                logged.set(false);
            }
        }
    }

    let on_login_event = {
        let storage = storage.clone();
        let logged = logged.clone();
        Callback::from(move |event: LoginFormData| {
            let storage = storage.clone();
            let logged = logged.clone();
            spawn_local(async move {
                match auth_api::login(event.username.clone(), event.password.clone()).await {
                    Ok(response) => {
                        storage.set(UserSession {
                            user_id: response.user_id,
                            token: response.access_token.clone(),
                            username: response.username.clone(),
                        });
                        logged.set(true);
                        log::info!(
                            "User login successful, username: {username}, user_id: {user_id}",
                            username = &response.username,
                            user_id = &response.user_id
                        );
                    }
                    Err(error) => {
                        log::warn!("Login failed, error: {error}");
                        storage.delete();
                        logged.set(false);
                    }
                }
            });
        })
    };

    let on_logout = {
        let storage = storage.clone();
        let logged = logged.clone();
        Callback::from(move |_| {
            log::info!("User logged out");
            storage.delete();
            logged.set(false);
        })
    };

    html! {
        if *logged {
            <Home user_session={storage.as_ref().expect("if logged is true, user session is some").clone()}
                  on_logout={on_logout} />
        } else {
            <main>
                <LoginForm on_login={on_login_event}/>
            </main>
        }
    }
}
