use ammonia::clean;
use yew::{function_component, AttrValue, Html, Properties};

#[derive(Properties, PartialEq)]
pub struct Props {
    pub html: String,
}

#[function_component(BlockquoteHtml)]
pub fn blockquote_html(props: &Props) -> Html {
    let html = format!("<blockquote>{}</blockquote>", clean(&props.html));
    Html::from_html_unchecked(AttrValue::from(html))
}

#[function_component(ArticleHtml)]
pub fn article_html(props: &Props) -> Html {
    let html = format!("<article>{}</article>", clean(&props.html));
    Html::from_html_unchecked(AttrValue::from(html))
}
