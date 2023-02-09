use crate::router::Route;
use yew::prelude::*;
use yew_router::prelude::*;

#[function_component(NotFound)]
pub fn not_found() -> Html {
    html! {
        <>
            <p>{"Page not found!"}</p>
            <Link<Route> to={Route::Home}>{ "click here to go home" }</Link<Route>>
        </>
    }
}
