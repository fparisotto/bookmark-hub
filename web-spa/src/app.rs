use yew::prelude::*;
use yew_router::prelude::*;
use yewdux::prelude::*;

use crate::{
    api::auth_api,
    router::{switch, Route},
    user_session::UserSession,
};

#[function_component(App)]
pub fn app() -> Html {
    let (store, dispatch) = use_store::<UserSession>();

    let token = store.token.clone();
    let is_loaded = use_state(|| false);
    let is_logged = store.logged();

    use_effect(move || {
        log::info!("App is_loaded={}, is_logged={}", *is_loaded, is_logged);
        if is_logged && !(*is_loaded) {
            let dispatch = dispatch.clone();
            let is_loaded = is_loaded.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match auth_api::get_user_profile(token).await {
                    Ok(user_profile) => {
                        log::info!(
                            "User profile logged {}, store={:?}",
                            serde_json::to_string(&user_profile).unwrap(),
                            *store
                        );
                        is_loaded.set(true);
                    }
                    Err(error) => {
                        log::warn!("Fail to fetch user profile, login out! error={}", error);
                        UserSession::logout(dispatch.clone());
                    }
                }
            });
        }
        || {}
    });

    html! {
        <BrowserRouter>
            <Switch<Route> render={switch} />
        </BrowserRouter>
    }
}
