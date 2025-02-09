use anyhow::bail;
use chrono::{DateTime, FixedOffset, NaiveDateTime, Utc};
use wasm_bindgen::JsCast;
use web_sys::{js_sys, EventTarget, HtmlInputElement};
use yew::prelude::*;

#[derive(PartialEq, Properties, Default, Debug)]
pub struct Props {
    pub id: String,
    pub name: String,
    pub value: Option<DateTime<Utc>>,
    #[prop_or_default]
    pub class: Classes,
    pub on_change: Callback<Option<DateTime<Utc>>>,
}

#[function_component(InputDateTimeUtc)]
pub fn input_datetime_utc(props: &Props) -> Html {
    let callback = props.on_change.clone();

    let value_string = props
        .value
        .map(|e| {
            e.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
                .replace("Z", "")
        })
        .unwrap_or_default();

    let on_change = {
        Callback::from(move |event: Event| {
            let target: EventTarget = event.target().expect("Fail to cast to EventTarget");
            let value_string: String = target.unchecked_into::<HtmlInputElement>().value();
            if value_string.trim().is_empty() {
                callback.emit(None);
            } else {
                match local_to_utc(&value_string) {
                    Ok(datetime_utc) => {
                        callback.emit(Some(datetime_utc));
                    }
                    Err(error) => {
                        log::error!(
                        "Fail to convert date to utc, string: {value_string}, error: {error}, doing nothing"
                    );
                    }
                }
            }
        })
    };

    html! {
        <input
            id={props.id.clone()}
            name={props.name.clone()}
            value={value_string}
            type="datetime-local"
            class={props.class.clone()}
            onchange={on_change} />
    }
}

fn local_to_utc(raw_datetime: &str) -> anyhow::Result<DateTime<Utc>> {
    let naive_parsed =
        NaiveDateTime::parse_from_str(raw_datetime, "%Y-%m-%dT%H:%M").map_err(|error| {
            anyhow::anyhow!("Fail to parse naive date: {raw_datetime}, error: {error}")
        })?;
    let offset_in_minutes = js_sys::Date::new_0().get_timezone_offset() as i32;
    let timezone = FixedOffset::east_opt(offset_in_minutes * 60).ok_or_else(|| {
        anyhow::anyhow!("Fail to convert offset from js, value: {offset_in_minutes}")
    })?;
    let datetime = match naive_parsed.and_local_timezone(timezone) {
        chrono::offset::LocalResult::Single(datetime) => datetime,
        chrono::offset::LocalResult::Ambiguous(earliest, latest) => {
            bail!("Ambiguous dates, earliest: {earliest}, latest: {latest}")
        }
        chrono::offset::LocalResult::None => bail!("Ambiguous dates, input: {raw_datetime}"),
    };
    Ok(datetime.to_utc())
}
