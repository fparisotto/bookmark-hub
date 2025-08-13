use std::str::FromStr;

use chrono::{DateTime, Utc};
use shared::{BookmarkTaskSearchRequest, BookmarkTaskStatus};
use yew::prelude::*;

use crate::components::atoms::input_datetime_utc::InputDateTimeUtc;
use crate::components::atoms::input_text::{InputText, InputType};
use crate::components::atoms::select::Select;

#[derive(PartialEq, Properties)]
pub struct Props {
    pub on_submit: Callback<BookmarkTaskSearchRequest>,
}

#[function_component(TasksFilter)]
pub fn tasks_filter(props: &Props) -> Html {
    let state_handle = use_state(|| BookmarkTaskSearchRequest {
        status: Some(Default::default()),
        page_size: Some(25),
        ..Default::default()
    });

    let tags_as_string = state_handle.tags.to_owned().unwrap_or_default().join(", ");

    let on_tag_change: Callback<String> = {
        let state_handle = state_handle.clone();
        Callback::from(move |text: String| {
            let value = if text.trim().is_empty() {
                None
            } else {
                Some(text.split(',').map(|t| t.trim().to_owned()).collect())
            };
            let mut state = (*state_handle).clone();
            state.tags = value;
            state_handle.set(state);
        })
    };

    let on_url_change = {
        let state_handle = state_handle.clone();
        Callback::from(move |text: String| {
            let value = if text.trim().is_empty() {
                None
            } else {
                Some(text)
            };
            let mut state = (*state_handle).clone();
            state.url = value;
            state_handle.set(state);
        })
    };

    let created_to_on_change = {
        let state_handle = state_handle.clone();
        Callback::from(move |datetime_utc: Option<DateTime<Utc>>| {
            let mut state = (*state_handle).clone();
            state.to_created_at = datetime_utc;
            state_handle.set(state);
        })
    };

    let created_from_on_change = {
        let state_handle = state_handle.clone();
        Callback::from(move |datetime_utc: Option<DateTime<Utc>>| {
            let mut state = (*state_handle).clone();
            state.from_created_at = datetime_utc;
            state_handle.set(state);
        })
    };

    let on_submit = {
        let on_submit = props.on_submit.clone();
        let state_handle = state_handle.clone();
        Callback::from(move |event: SubmitEvent| {
            event.prevent_default();
            let state = (*state_handle).clone();
            on_submit.emit(state);
        })
    };

    let tasks_status_options: Vec<String> = vec![
        BookmarkTaskStatus::Done.as_ref().to_owned(),
        BookmarkTaskStatus::Pending.as_ref().to_owned(),
        BookmarkTaskStatus::Fail.as_ref().to_owned(),
    ];
    let selected_tasks_status = state_handle
        .status
        .to_owned()
        .map(|e| e.as_ref().to_owned());

    let on_task_status_change = {
        let state_handle = state_handle.clone();
        Callback::from(move |text: String| {
            let mut state = (*state_handle).clone();
            match BookmarkTaskStatus::from_str(&text) {
                Ok(task_status) => {
                    state.status = Some(task_status);
                }
                Err(error) => {
                    state.status = None;
                    // FIXME: notify user
                    log::error!("Fail to parse BookmarkTaskStatus, error: {error}");
                }
            }
            state_handle.set(state);
        })
    };

    let on_page_size_change = {
        let state_handle = state_handle.clone();
        Callback::from(move |text: String| {
            let mut state = (*state_handle).clone();
            if let Ok(page_size) = text.parse::<u8>() {
                state.page_size = Some(page_size);
                // Reset pagination when page size changes
                state.last_task_id = None;
            }
            state_handle.set(state);
        })
    };

    let page_size_options: Vec<String> = vec![
        "10".to_string(),
        "25".to_string(),
        "50".to_string(),
        "100".to_string(),
    ];
    let selected_page_size = state_handle.page_size.unwrap_or(25).to_string();

    html! {
        <form class="row g-3 align-items-end mb-4" onsubmit={on_submit}>
          <div class="col-md-1">
            <label for="status" class="form-label mb-1">{"Status"}</label>
            <Select
                id="status"
                name="status"
                class="form-select form-select-sm"
                options={tasks_status_options}
                selected={selected_tasks_status}
                on_change={on_task_status_change} />
          </div>

          <div class="col-md-2">
            <label for="created_from" class="form-label mb-1">{"Created From"}</label>
            <InputDateTimeUtc
                id="created_from"
                name="created_from"
                value={state_handle.from_created_at.to_owned()}
                on_change={created_from_on_change}
                class="form-control form-control-sm" />
          </div>

          <div class="col-md-2">
            <label for="created_to" class="form-label mb-1">{"Created To"}</label>
            <InputDateTimeUtc
                id="created_to"
                name="created_to"
                value={state_handle.to_created_at.to_owned()}
                on_change={created_to_on_change}
                class="form-control form-control-sm" />
          </div>

          <div class="col-md-2">
            <label for="url_search" class="form-label mb-1">{"URL"}</label>
            <InputText
                id="url_search"
                name="url_search"
                placeholder="Search by URL (contains this text)"
                class="form-control form-control-sm"
                input_type={InputType::Text}
                on_change={on_url_change}
                value={state_handle.url.clone()} />
          </div>

          <div class="col-md-2">
            <label for="tags_search" class="form-label mb-1">{"Tags"}</label>
            <InputText
                id="tags_search"
                name="tags_search"
                placeholder="Search by Tags"
                class="form-control form-control-sm"
                input_type={InputType::Text}
                on_change={on_tag_change}
                value={tags_as_string} />
          </div>

          <div class="col-md-1">
            <label for="page_size" class="form-label mb-1">{"Page Size"}</label>
            <Select
                id="page_size"
                name="page_size"
                class="form-select form-select-sm"
                options={page_size_options}
                selected={Some(selected_page_size)}
                on_change={on_page_size_change} />
          </div>

          <div class="col-md-1">
            <input type="submit" class="btn btn-primary btn-sm w-100" value="Filter" />
          </div>
        </form>
    }
}
