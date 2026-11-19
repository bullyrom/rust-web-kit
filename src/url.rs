use url::Url;

use crate::error::{ErrorResponse, FieldError};

const TRACKING_PARAM_PREFIXES: [&str; 3] = ["utm_", "pk_", "mc_"];
const TRACKING_PARAMS: [&str; 8] = [
    "fbclid",
    "gclid",
    "yclid",
    "msclkid",
    "igshid",
    "mc_cid",
    "mc_eid",
    "_ga",
];
const SUPPORTED_SCHEMES: [&str; 2] = ["http", "https"];
const HTTP_DEFAULT_PORT: u16 = 80;
const HTTPS_DEFAULT_PORT: u16 = 443;
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
const HASH_PART_SEPARATOR: u8 = 0xff;

pub fn canonicalize_url(raw: &str) -> Result<String, ErrorResponse> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(invalid_url("URL must not be empty"));
    }

    let mut url = Url::parse(raw).map_err(|error| invalid_url(format!("Invalid URL: {error}")))?;

    if !SUPPORTED_SCHEMES.contains(&url.scheme()) {
        return Err(invalid_url("Only http and https URLs are supported"));
    }

    url.set_fragment(None);

    if let Some(host) = url.host_str().map(str::to_lowercase) {
        url.set_host(Some(&host))
            .map_err(|error| invalid_url(format!("Invalid URL host: {error}")))?;
    }

    match (url.scheme(), url.port()) {
        ("http", Some(HTTP_DEFAULT_PORT)) | ("https", Some(HTTPS_DEFAULT_PORT)) => {
            let _ = url.set_port(None);
        }
        _ => {}
    }

    let mut query_pairs: Vec<(String, String)> = url
        .query_pairs()
        .into_owned()
        .filter(|(key, _)| !is_tracking_param(key))
        .collect();
    query_pairs.sort();

    if query_pairs.is_empty() {
        url.set_query(None);
    } else {
        let mut serializer = url.query_pairs_mut();
        serializer.clear();
        serializer.extend_pairs(query_pairs);
    }

    let path = url.path().to_string();
    if path.len() > 1 && path.ends_with('/') {
        url.set_path(path.trim_end_matches('/'));
    }

    Ok(url.to_string())
}

pub fn content_hash(parts: &[&str]) -> String {
    let mut hash = FNV_OFFSET_BASIS;

    for part in parts {
        for byte in part.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        hash ^= u64::from(HASH_PART_SEPARATOR);
        hash = hash.wrapping_mul(FNV_PRIME);
    }

    format!("{hash:016x}")
}

fn is_tracking_param(key: &str) -> bool {
    let key = key.to_lowercase();
    TRACKING_PARAMS.contains(&key.as_str())
        || TRACKING_PARAM_PREFIXES
            .iter()
            .any(|prefix| key.starts_with(prefix))
}

fn invalid_url(reason: impl Into<String>) -> ErrorResponse {
    ErrorResponse::validation(vec![FieldError::new("url", "invalid", reason)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_tracking_parameters_fragment_and_default_port() {
        let canonical = canonicalize_url(
            "HTTPS://Example.COM:443/items/item/?utm_source=x&b=2&fbclid=abc&a=1#top",
        )
        .expect("URL must canonicalize");

        assert_eq!(canonical, "https://example.com/items/item?a=1&b=2");
    }

    #[test]
    fn keeps_meaningful_query_parameters_and_sort_order() {
        let canonical = canonicalize_url("https://example.com/search?q=rust&page=2")
            .expect("URL must canonicalize");

        assert_eq!(canonical, "https://example.com/search?page=2&q=rust");
    }

    #[test]
    fn strips_a_trailing_slash_but_keeps_the_root() {
        assert_eq!(
            canonicalize_url("https://example.com/items/").expect("URL must canonicalize"),
            "https://example.com/items"
        );
        assert_eq!(
            canonicalize_url("https://example.com/").expect("URL must canonicalize"),
            "https://example.com/"
        );
    }

    #[test]
    fn rejects_non_http_schemes_and_empty_values() {
        assert!(canonicalize_url("ftp://example.com/file").is_err());
        assert!(canonicalize_url("   ").is_err());
    }

    #[test]
    fn content_hash_is_stable_and_changes_with_content() {
        let first = content_hash(&["https://example.com", "Title", "Body"]);
        let same = content_hash(&["https://example.com", "Title", "Body"]);
        let changed = content_hash(&["https://example.com", "Title", "Other body"]);

        assert_eq!(first, same);
        assert_ne!(first, changed);
        assert_eq!(first.len(), 16);
    }
}
