use yew::prelude::*;

#[derive(Debug, Clone, PartialEq, Properties)]
pub struct Props {
    pub has_more: bool,
    pub on_previous: Callback<()>,
    pub on_next: Callback<()>,
    pub current_page: usize,
    pub page_size: usize,
    pub current_count: usize,
}

#[function_component(PaginationControls)]
pub fn pagination_controls(props: &Props) -> Html {
    let on_previous = {
        let on_previous = props.on_previous.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            on_previous.emit(());
        })
    };

    let on_next = {
        let on_next = props.on_next.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            on_next.emit(());
        })
    };

    let start_index = (props.current_page - 1) * props.page_size + 1;
    let end_index = start_index + props.current_count - 1;

    html! {
        <nav aria-label="Tasks pagination">
            <div class="d-flex justify-content-between align-items-center">
                <div class="text-muted">
                    {format!("Showing {} - {} items", start_index, end_index)}
                </div>
                <ul class="pagination mb-0">
                    <li class={if props.current_page <= 1 { "page-item disabled" } else { "page-item" }}>
                        <a class="page-link" href="#" onclick={on_previous} aria-label="Previous">
                            <span aria-hidden="true">{"«"}</span>
                        </a>
                    </li>
                    <li class="page-item active">
                        <span class="page-link">
                            {format!("Page {}", props.current_page)}
                        </span>
                    </li>
                    <li class={if !props.has_more { "page-item disabled" } else { "page-item" }}>
                        <a class="page-link" href="#" onclick={on_next} aria-label="Next">
                            <span aria-hidden="true">{"»"}</span>
                        </a>
                    </li>
                </ul>
            </div>
        </nav>
    }
}
