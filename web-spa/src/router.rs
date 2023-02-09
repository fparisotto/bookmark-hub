use yew::prelude::*;
use yew_router::prelude::*;

use crate::pages::{bookmark::BookmarkPage, home::Home, login::Login, not_found::NotFound};

#[derive(Clone, Routable, PartialEq)]
pub enum Route {
    #[at("/")]
    Home,
    #[at("/login")]
    Login,
    #[at("/bookmark/:id")]
    Bookmark { id: String },
    #[not_found]
    #[at("/404")]
    NotFound,
}

pub fn switch(route: Route) -> Html {
    match route {
        Route::Home => html! { <Home /> },
        Route::Bookmark { id } => html! { <BookmarkPage bookmark_id={id} /> },
        Route::Login => html! { <Login /> },
        Route::NotFound => html! { <NotFound /> },
    }
}
