use pulldown_cmark::{html, Options, Parser};
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct MarkdownProps {
    pub content: String,
    #[prop_or_default]
    pub class: Option<String>,
}

#[function_component(MarkdownRender)]
pub fn markdown_render(props: &MarkdownProps) -> Html {
    let html_output = markdown_to_html(&props.content);

    let class = props
        .class
        .clone()
        .unwrap_or_else(|| "markdown-content".to_string());

    html! {
        <div class={class}>
            {Html::from_html_unchecked(html_output.into())}
        </div>
    }
}

fn markdown_to_html(markdown: &str) -> String {
    // Enable common markdown extensions
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_TASKLISTS);

    // Create parser with the markdown input
    let parser = Parser::new_ext(markdown, options);

    // Write to String buffer
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);

    html_output
}
