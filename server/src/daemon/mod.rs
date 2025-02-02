use anyhow::{anyhow, bail, Result};
use murmur3::murmur3_x64_128;
use std::io::Cursor;
use url::Url;

mod processor;
mod runner;

pub use self::runner::run;

fn clean_url(url: Url) -> Result<Url> {
    if let Some(host) = url.host_str() {
        let path = &url.path();
        let clean_url = format!("{scheme}://{host}{path}", scheme = &url.scheme());
        tracing::info!("Clean url={clean_url}");
        let clean = Url::parse(&clean_url)?;
        return Ok(clean);
    }
    bail!("Invalid url={url}")
}

fn make_bookmark_id(url: &Url) -> Result<String> {
    if let Some(host) = url.host_str() {
        let path = url.path();
        let source = format!("{host}.{path}");
        let mut source = Cursor::new(source.as_str());
        let hash = murmur3_x64_128(&mut source, 0)?;
        let id = base64_url::encode(&hash.to_be_bytes());
        tracing::info!(id = &id, url = format!("{url}"), "Making url id");
        return Ok(id);
    }
    bail!("Invalid url={url}")
}

fn domain_from_url(url: &Url) -> Result<String> {
    let domain_or_host = url
        .domain()
        .or_else(|| url.host_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!(format!("Domain not found for url={url}")))?;
    Ok(domain_or_host)
}
