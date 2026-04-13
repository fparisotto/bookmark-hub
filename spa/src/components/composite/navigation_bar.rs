use yew::prelude::*;

use crate::router::{self, AppRoute, RouteKind};

#[derive(PartialEq, Properties)]
pub struct Props {
    pub username: String,
    pub active_page: RouteKind,
    pub on_page_change: Callback<RouteKind>,
    pub on_logout: Callback<()>,
}

#[function_component(NavigationBar)]
pub fn navigation_bar(props: &Props) -> Html {
    let render_nav_link =
        |label: &'static str, route_kind: RouteKind, route: AppRoute, active: RouteKind| {
            let href = router::href(&route);
            let classes = if active == route_kind {
                classes!("nav-link", "active")
            } else {
                classes!("nav-link")
            };
            let on_page_change = props.on_page_change.clone();
            let onclick = Callback::from(move |event: MouseEvent| {
                if router::should_handle_spa_navigation(&event) {
                    event.prevent_default();
                    on_page_change.emit(route_kind);
                }
            });

            html! {
                <li class="nav-item">
                    <a href={href} onclick={onclick} class={classes}>{label}</a>
                </li>
            }
        };

    let home_href = router::href(&AppRoute::Search(Default::default()));
    let on_home_click = {
        let on_page_change = props.on_page_change.clone();
        Callback::from(move |event: MouseEvent| {
            if router::should_handle_spa_navigation(&event) {
                event.prevent_default();
                on_page_change.emit(RouteKind::Search);
            }
        })
    };

    let on_logout_click = {
        let on_logout = props.on_logout.clone();
        Callback::from(move |_| {
            on_logout.emit(());
        })
    };

    html! {
        <nav class="navbar navbar-expand-lg bg-body-tertiary">
            <div class="container-fluid">
                <a class="navbar-brand" href={home_href} onclick={on_home_click}>{"BookMark Hub"}</a>
                <div class="collapse navbar-collapse">
                    <ul class="navbar-nav me-auto mb-2 mb-lg-0">
                        {render_nav_link("Search", RouteKind::Search, AppRoute::Search(Default::default()), props.active_page)}
                        {render_nav_link("Tasks", RouteKind::Tasks, AppRoute::Tasks, props.active_page)}
                        {render_nav_link("AI Search", RouteKind::RAG, AppRoute::RAG, props.active_page)}
                        {render_nav_link("AI History", RouteKind::RagHistory, AppRoute::RagHistory, props.active_page)}
                    </ul>
                    <button class="btn btn-sm me-2 btn-outline-primary" data-bs-toggle="modal" data-bs-target="#add-bookmark-modal">
                        {"+ Bookmark"}
                    </button>
                    <span class="navbar-text me-3">{&props.username}</span>
                    <button onclick={on_logout_click} class="btn btn-sm btn-outline-secondary">
                        {"Logout"}
                    </button>
                </div>
            </div>
        </nav>
    }
}
