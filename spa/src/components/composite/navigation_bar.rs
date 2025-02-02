use yew::prelude::*;

#[derive(PartialEq, Properties)]
pub struct Props {
    pub email: String,
}

#[function_component(NavigationBar)]
pub fn navigation_bar(props: &Props) -> Html {
    html! {
        <nav class="navbar navbar-expand-lg bg-body-tertiary">
            <div class="container-fluid">
                <a class="navbar-brand" href="#">{"BookMark Hub"}</a>
                <div class="collapse navbar-collapse" id="navbarNav">
                    <ul class="navbar-nav ms-auto">
                        <li class="nav-item">
                            <button class="btn btn-primary" data-bs-toggle="modal" data-bs-target="#add-bookmark-modal">
                                {"Add Bookmark"}
                            </button>
                        </li>
                        <li class="nav-item">
                            <span class="nav-link">{&props.email}</span>
                        </li>
                    </ul>
                </div>
            </div>
        </nav>
    }
}
