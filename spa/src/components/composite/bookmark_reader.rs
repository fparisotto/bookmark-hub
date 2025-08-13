use log::warn;
use shared::Bookmark;
use yew::platform::spawn_local;
use yew::prelude::*;

use crate::api::bookmarks_api;
use crate::components::atoms::input_text::{InputText, InputType};
use crate::components::atoms::safe_html::ArticleHtml;
use crate::user_session::UserSession;

#[derive(Debug, Clone, PartialEq, Properties)]
pub struct Props {
    pub user_session: UserSession,
    pub bookmark: Bookmark,
    pub on_goback: Callback<()>,
    pub on_new_tags: Callback<Vec<String>>,
}

#[function_component(BookmarkReader)]
pub fn bookmark_page(props: &Props) -> Html {
    let token = props.user_session.token.clone();
    let user_id = props.user_session.user_id;
    let state = use_state_eq(|| props.bookmark.tags.clone().unwrap_or_default());
    let tags_as_string = state.clone().join(", ");
    let html_content = use_state(|| None);
    {
        let html_contentt = html_content.clone();
        let token = token.clone();
        let bookmark_id = props.bookmark.bookmark_id.clone();
        use_effect_with(bookmark_id.clone(), move |_| {
            spawn_local(async move {
                match bookmarks_api::get_content(&token, &user_id, &bookmark_id).await {
                    Ok(Some(data)) => html_contentt.set(Some(data)),
                    Ok(None) => html_contentt.set(Some("".to_owned())),
                    Err(error) => {
                        warn!(
                            "Failed to fetch static content from bookmark_id: {}, user_id: {}, error: {}",
                            &bookmark_id, user_id, error
                        );
                    }
                }
            });
        });
    }

    let on_tag_change: Callback<String> = {
        let state = state.clone();
        Callback::from(move |text: String| {
            let new_tags: Vec<String> = text.split(',').map(|t| t.trim().into()).collect();
            state.set(new_tags);
        })
    };

    let on_goback = {
        let callback = props.on_goback.clone();
        Callback::from(move |event: MouseEvent| {
            event.prevent_default();
            callback.emit(());
        })
    };

    let on_save_tags = {
        let callback = props.on_new_tags.clone();
        Callback::from(move |_: MouseEvent| {
            callback.emit((*state).clone());
        })
    };

    let article = if let Some(data) = (*html_content).clone() {
        html! {
            <ArticleHtml html={data} />
        }
    } else {
        html! { "Loading..." }
    };

    let summary = if let Some(summary) = &props.bookmark.summary {
        html! {
            <>
                <h4>{"Summary"}</h4>
                <p><em>{summary}</em></p>
            </>
        }
    } else {
        html! { <></> }
    };

    html! {
      <div class="container mt-5">
          <div class="mb-3">
              <a href="#" class="btn btn-secondary" onclick={on_goback.clone()}>{"< Back to Home"}</a>
          </div>
          <div class="card">
              <div class="card-body">
                  <div>
                      <div class="d-flex justify-content-between align-items-center">
                          <p class="mb-0"><strong>{"ID:"}</strong>{" "}{props.bookmark.bookmark_id.clone()}</p>
                          <p class="mb-0"><strong>{"Created at:"}</strong>{" "}{props.bookmark.created_at}</p>
                      </div>
                      <p class="mb-2">
                          <strong>{"Original URL:"}</strong>
                          <a href={ props.bookmark.url.clone() } target="_blank">
                              { props.bookmark.url.clone() }
                          </a>
                      </p>
                      <div class="input-group">
                          <label for="tags" class="input-group-text">{"Tags:"}</label>
                          <InputText
                              id="tags"
                              name="tags"
                              placeholder="Tags"
                              class={classes!("form-control")}
                              input_type={InputType::Text}
                              on_change={on_tag_change}
                              value={tags_as_string} />
                          <button onclick={on_save_tags} class="btn btn-primary" type="button">{"Save"}</button>
                      </div>
                  </div>
              </div>
          </div>
          <br/>
          <div>
            <style>
            {"
                figure img {
                  max-width: 100%;
                  height: auto;
                }
            "}
            </style>
            {summary}
            <h1 class="mb-4">{ props.bookmark.title.clone() }</h1>
            {article}
          </div>
          <div class="mb-3">
              <a href="#" class="btn btn-secondary" onclick={on_goback.clone()}>{"< Back to Home"}</a>
          </div>
      </div>
    }
}
