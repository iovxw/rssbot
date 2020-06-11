use std::env;
use std::time::Duration;

use once_cell::sync::OnceCell;
use reqwest::{
    self,
    header::{HeaderValue, CONTENT_TYPE},
};
use thiserror::Error;

use crate::feed::Rss;

static RESP_SIZE_LIMIT: OnceCell<u64> = OnceCell::new();
static CLIENT: OnceCell<reqwest::Client> = OnceCell::new();

#[derive(Error, Debug)]
pub enum FeedError {
    #[error("network error")]
    Network(#[from] reqwest::Error),
    #[error("feed parsing failed")]
    Parsing(#[from] quick_xml::Error),
    #[error("feed is too large")]
    TooLarge(u64),
}

impl FeedError {
    pub fn to_user_friendly(&self) -> String {
        match self {
            Self::Network(source) => tr!("network_error", source = source),
            Self::Parsing(source) => tr!("parsing_error", source = source),
            Self::TooLarge(limit) => tr!(
                "rss_size_limit_exceeded",
                size = format_byte_size((*limit).into())
            ),
        }
    }
}

pub async fn pull_feed(url: &str) -> Result<Rss, FeedError> {
    let mut resp = CLIENT
        .get()
        .expect("CLIENT not initialized")
        .get(url)
        .send()
        .await?
        .error_for_status()?;
    let size_limit = *RESP_SIZE_LIMIT
        .get()
        .expect("RESP_SIZE_LIMIT not initialized");
    let unlimited = size_limit == 0;
    if let Some(len) = resp.content_length() {
        if !unlimited && len > size_limit {
            return Err(FeedError::TooLarge(size_limit));
        }
    }

    let feed = if url.ends_with(".json")
        || matches!(
            resp.headers().get(CONTENT_TYPE),
            Some(v) if content_type_is_json(v)
        ) {
        resp.json().await?
    } else {
        let mut buf = Vec::new(); // TODO: capacity?
        while let Some(bytes) = resp.chunk().await? {
            if !unlimited && buf.len() + bytes.len() > size_limit as usize {
                return Err(FeedError::TooLarge(size_limit));
            }
            buf.extend_from_slice(&bytes);
        }

        crate::feed::parse(std::io::Cursor::new(buf))?
    };

    Ok(crate::feed::fix_relative_url(feed, url))
}

pub fn init_client(bot_name: &str, insecue: bool, max_feed_size: u64) {
    let mut headers = reqwest::header::HeaderMap::new();
    let ua = format!(
        concat!(
            env!("CARGO_PKG_NAME"),
            "/",
            env!("CARGO_PKG_VERSION"),
            " (+https://t.me/{})"
        ),
        bot_name
    );
    headers.insert(
        reqwest::header::USER_AGENT,
        reqwest::header::HeaderValue::from_str(&ua).unwrap(),
    );
    let mut client_builder = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .default_headers(headers)
        .danger_accept_invalid_certs(insecue)
        .redirect(reqwest::redirect::Policy::limited(5));

    if env::var("RSSBOT_DONT_PROXY_FEEDS")
        .or_else(|_| env::var("rssbot_dont_proxy_feeds"))
        .is_ok()
    {
        client_builder = client_builder.no_proxy();
    }

    let client = client_builder.build().unwrap();

    CLIENT.set(client).expect("CLIENT already initialized");
    RESP_SIZE_LIMIT
        .set(max_feed_size)
        .expect("RESP_SIZE_LIMIT already initialized");
}

fn content_type_is_json(value: &HeaderValue) -> bool {
    value
        .to_str()
        .map(|value| {
            value
                .split(';')
                .map(|v| v.trim())
                .any(|v| v == "application/json")
        })
        .unwrap_or(false)
}

/// About the "kiB" not "KiB": https://en.wikipedia.org/wiki/Metric_prefix#List_of_SI_prefixes
fn format_byte_size(bytes: u64) -> String {
    const SIZES: [&str; 7] = ["B", "kiB", "MiB", "GiB", "TiB", "PiB", "EiB"];
    const BASE: f64 = 1024.0;

    if bytes == 0 {
        return "0B".into();
    }
    let bytes = bytes as f64;

    // `ln` is faster than `log2` in glibc:
    // test ln   ... bench:      55,206 ns/iter (+/- 2,832)
    // test log2 ... bench:      78,612 ns/iter (+/- 4,427)
    //
    // And musl:
    // test ln   ... bench:      47,276 ns/iter (+/- 1,205)
    // test log2 ... bench:      50,671 ns/iter (+/- 3,485)
    let i = (bytes.ln() / BASE.ln()).floor() as i32;
    let divisor = BASE.powi(i);

    // Don't worry the out of bounds, u64::MAX is only 16EiB
    format!("{:.0}{}", (bytes / divisor), SIZES[i as usize])
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn max_format_byte_size() {
        assert_eq!(format_byte_size(std::u64::MAX), "16EiB");
    }

    #[test]
    fn format_byte_size_zero() {
        assert_eq!(format_byte_size(0), "0B");
    }

    #[test]
    fn format_byte_size_normal() {
        assert_eq!(format_byte_size(1), "1B");
        assert_eq!(format_byte_size(10), "10B");
        assert_eq!(format_byte_size(1024), "1kiB");
        assert_eq!(format_byte_size(1024 * 10), "10kiB");
        assert_eq!(format_byte_size(1024 * 1024), "1MiB");
        assert_eq!(format_byte_size(1024 * 1024 * 10), "10MiB");

        assert_eq!(format_byte_size(1024 + 10), "1kiB");
    }
}
