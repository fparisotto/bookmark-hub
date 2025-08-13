use yew::prelude::*;

use crate::pages::home::Page;

#[derive(PartialEq, Properties)]
pub struct Props {
    pub username: String,
    pub active_page: Page,
    pub on_page_change: Callback<Page>,
    pub on_logout: Callback<()>,
}

#[function_component(NavigationBar)]
pub fn navigation_bar(props: &Props) -> Html {
    let on_search_click = {
        let on_page_change = props.on_page_change.clone();
        Callback::from(move |_| {
            on_page_change.emit(Page::Search);
        })
    };

    let on_tasks_click = {
        let on_page_change = props.on_page_change.clone();
        Callback::from(move |_| {
            on_page_change.emit(Page::Tasks);
        })
    };

    let on_logout_click = {
        let on_logout = props.on_logout.clone();
        Callback::from(move |_| {
            on_logout.emit(());
        })
    };

    let search_classes = if props.active_page == Page::Search {
        classes!("nav-link", "active")
    } else {
        classes!("nav-link")
    };
    let task_classes = if props.active_page == Page::Tasks {
        classes!("nav-link", "active")
    } else {
        classes!("nav-link")
    };

    html! {
        <nav class="navbar navbar-expand-lg bg-body-tertiary">
            <div class="container-fluid">
                <a class="navbar-brand" href="#">{"BookMark Hub"}</a>
                <div class="collapse navbar-collapse">
                    <ul class="navbar-nav me-auto mb-2 mb-lg-0">
                        <li class="nav-item">
                            <a onclick={on_search_click} class={search_classes}>{"Search"}</a>
                        </li>
                        <li class="nav-item">
                            <a onclick={on_tasks_click} class={task_classes}>{"Tasks"}</a>
                        </li>
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
