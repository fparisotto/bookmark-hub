use crate::{
    api::{self, bookmarks_api::Bookmark},
    components::atoms::{
        input_text::{InputText, InputType},
        safe_html::ArticleHtml,
    },
    user_session::UserSession,
};
use yew::platform::spawn_local;
use yew::prelude::*;

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
    let state = use_state_eq(|| props.bookmark.tags.clone().unwrap_or_default());
    let tags_as_string = state.clone().join(", ");
    let html_content = use_state(|| None);
    {
        let html_contentt = html_content.clone();
        let token = token.clone();
        let bookmark_id = props.bookmark.bookmark_id.clone();
        use_effect_with_deps(
            move |_| {
                spawn_local(async move {
                    match api::bookmarks_api::get_content(&token, &bookmark_id).await {
                        Ok(Some(data)) => html_contentt.set(Some(data)),
                        Ok(None) => todo!(),
                        Err(_) => todo!(),
                    }
                });
                || ()
            },
            (),
        );
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

    html! {
      <div class="container mt-5">
          <div class="mb-3">
              <a href="#" class="btn btn-secondary" onclick={on_goback}>{"< Back to Home"}</a>
          </div>
          <div class="card">
              <div class="card-body">
                  <div class="mb-4">
                      <div class="d-flex justify-content-between align-items-center">
                          <p class="mb-0"><strong>{"ID:"}</strong> {props.bookmark.bookmark_id.clone()}</p>
                          <p class="mb-0"><strong>{"Created at:"}</strong> {props.bookmark.user_created_at}</p>
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
          <div class="article-content">
              <h1 class="mb-4">{ props.bookmark.title.clone() }</h1>
              {article}
          </div>
      </div>
    }
}
