use std::io::Cursor;

use anyhow::{anyhow, bail, Result};
use murmur3::murmur3_x64_128;
use tracing::info;
use url::Url;

pub fn canonicalize_url(mut url: Url) -> Result<Url> {
    if url.host_str().is_none() {
        bail!("Invalid url={url}");
    }

    url.set_fragment(None);

    if url.path().is_empty() {
        url.set_path("/");
    }

    let default_port = match url.scheme() {
        "http" => Some(80),
        "https" => Some(443),
        _ => None,
    };

    if url.port().is_some() && url.port() == default_port {
        url.set_port(None)
            .map_err(|_| anyhow!("Failed to normalize port for url={url}"))?;
    }

    Ok(url)
}

pub fn canonicalize_url_str(url: &str) -> Result<String> {
    let parsed = Url::parse(url)?;
    Ok(canonicalize_url(parsed)?.to_string())
}

pub fn make_bookmark_id(url: &Url) -> Result<String> {
    let canonical_url = canonicalize_url(url.clone())?;
    let mut source = Cursor::new(canonical_url.as_str());
    let hash = murmur3_x64_128(&mut source, 0)?;
    let id = base64_url::encode(&hash.to_be_bytes());
    info!(id = &id, url = canonical_url.as_str(), "Making url id");
    Ok(id)
}

pub fn domain_from_url(url: &Url) -> Result<String> {
    let canonical_url = canonicalize_url(url.clone())?;
    let domain_or_host = canonical_url
        .domain()
        .or_else(|| canonical_url.host_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!(format!("Domain not found for url={canonical_url}")))?;
    Ok(domain_or_host)
}

#[cfg(test)]
mod tests {
    use url::Url;

    use super::{canonicalize_url_str, make_bookmark_id};

    #[test]
    fn canonicalize_url_preserves_query_and_non_default_port() {
        let canonical =
            canonicalize_url_str("https://EXAMPLE.com:8443/post?a=1&b=2#section").unwrap();
        assert_eq!(canonical, "https://example.com:8443/post?a=1&b=2");
    }

    #[test]
    fn canonicalize_url_drops_default_port_and_fragment() {
        let canonical = canonicalize_url_str("https://EXAMPLE.com:443/post#section").unwrap();
        assert_eq!(canonical, "https://example.com/post");
    }

    #[test]
    fn bookmark_id_uses_full_canonical_url() {
        let a = Url::parse("https://example.com/post?a=1").unwrap();
        let b = Url::parse("https://example.com/post?a=2").unwrap();
        let c = Url::parse("http://example.com/post?a=1").unwrap();
        let d = Url::parse("https://example.com:8443/post?a=1").unwrap();

        assert_ne!(make_bookmark_id(&a).unwrap(), make_bookmark_id(&b).unwrap());
        assert_ne!(make_bookmark_id(&a).unwrap(), make_bookmark_id(&c).unwrap());
        assert_ne!(make_bookmark_id(&a).unwrap(), make_bookmark_id(&d).unwrap());
    }
}
