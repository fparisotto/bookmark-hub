use shared::{BookmarkTask, BookmarkTaskSearchResponse};
use yew::prelude::*;

#[derive(Debug, Clone, PartialEq, Properties)]
pub struct Props {
    pub response: Option<BookmarkTaskSearchResponse>,
}

fn render_bookmark_task(task: &BookmarkTask) -> Html {
    html! {
        <tr>
            <td>{task.task_id}</td>
            <td>{&task.url}</td>
            <td>{task.status.as_ref()}</td>
            <td>{&task.tags.join(", ")}</td>
            <td>{&task.created_at.to_rfc3339()}</td>
            <td>{&task.updated_at.to_rfc3339()}</td>
            <td>{&task.next_delivery.to_rfc3339()}</td>
            <td>{&task.retries.map(|e| e.to_string()).unwrap_or_default()}</td>
            <td>{&task.fail_reason.to_owned().unwrap_or_default()}</td>
        </tr>
    }
}

#[function_component(TasksTable)]
pub fn tasks_table(props: &Props) -> Html {
    let content = match &props.response {
        Some(response) => {
            let bookmark_tasks_html = response.tasks.iter().map(render_bookmark_task);
            html! { for bookmark_tasks_html }
        }
        None => {
            html! {
                <tr>
                    <td colspan="9" class="text-center text-muted">{"No data found"}</td>
                </tr>
            }
        }
    };

    html! {
        <table class="table table-striped table-hover">
            <thead>
              <tr>
                <th>{"Task ID"}</th>
                <th>{"URL"}</th>
                <th>{"Status"}</th>
                <th>{"Tags"}</th>
                <th>{"Created At"}</th>
                <th>{"Updated At"}</th>
                <th>{"Next Delivery"}</th>
                <th>{"Retries"}</th>
                <th>{"Fail Reason"}</th>
              </tr>
            </thead>
            <tbody>
                {content}
            </tbody>
        </table>
    }
}
