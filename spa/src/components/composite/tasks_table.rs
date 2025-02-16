use chrono::SecondsFormat;
use shared::{BookmarkTask, BookmarkTaskSearchResponse};
use yew::prelude::*;

#[derive(Debug, Clone, PartialEq, Properties)]
pub struct Props {
    pub response: Option<BookmarkTaskSearchResponse>,
}

fn render_fail_reason_modal(task_id: &str, fail_reason: &str) -> Html {
    let mini_text = format!("{}...", &fail_reason[0..12]);
    let modal_id = format!("modal-id-{}", task_id);
    let modal_id_ref = format!("#{}", modal_id);
    html! {
        <>
            <a href=""  data-bs-toggle="modal" data-bs-target={modal_id_ref}>{mini_text}</a>
            <div class="modal fade" id={modal_id} tabindex="-1" aria-labelledby="exampleModalLabel" aria-hidden="true">
              <div class="odal-dialog modal-dialog-centered modal-dialog-scrollable modal-lg">
                <div class="modal-content">
                  <div class="modal-header">
                    <h1 class="modal-title fs-5" id="exampleModalLabel">{"Fail reason"}</h1>
                    <button type="button" class="btn-close" data-bs-dismiss="modal" aria-label="Close"></button>
                  </div>
                  <div class="modal-body">
                    <pre>{fail_reason}</pre>
                  </div>
                  <div class="modal-footer">
                    <button type="button" class="btn btn-secondary" data-bs-dismiss="modal">{"Close"}</button>
                  </div>
                </div>
              </div>
            </div>
        </>
    }
}

fn render_bookmark_task(task: &BookmarkTask) -> Html {
    let task_id_strip = format!("{}...", &task.task_id.to_string()[0..6]);

    let fail_content = if let Some(fail_reason) = &task.fail_reason {
        render_fail_reason_modal(&task_id_strip, fail_reason)
    } else {
        html! {}
    };

    html! {
        <tr>
            <td>{task_id_strip}</td>
            <td><a href={task.url.to_owned()} target="_blank">{&task.url}</a></td>
            <td>{task.status.as_ref()}</td>
            <td>{&task.tags.to_owned().unwrap_or_default().join(", ")}</td>
            <td>{&task.created_at.to_rfc3339_opts(SecondsFormat::Secs, true)}</td>
            <td>{&task.updated_at.to_rfc3339_opts(SecondsFormat::Secs, true)}</td>
            <td>{&task.next_delivery.to_rfc3339_opts(SecondsFormat::Secs, true)}</td>
            <td>{&task.retries.map(|e| e.to_string()).unwrap_or_default()}</td>
            <td>{fail_content}</td>
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
