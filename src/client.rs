use std::env;
use std::sync::{Arc, Once};
use std::time::Duration;

use reqwest;
use thiserror::Error;

use crate::feed::Rss;

const RESP_SIZE_LIMIT: usize = 2 * 1024 * 1024;

#[derive(Error, Debug)]
pub enum FeedError {
    #[error("network error")]
    Network(#[from] reqwest::Error),
    #[error("feed parsing failed")]
    Parsing(#[from] quick_xml::Error),
    #[error("feed is too large")]
    TooLarge,
}

impl FeedError {
    pub fn to_user_friendly(&self) -> String {
        match self {
            Self::Network(source) => format!("网络错误（{}）", source),
            Self::Parsing(source) => format!("解析错误（{}）", source),
            Self::TooLarge => format!(
                "RSS 超出大小限制（{}）",
                format_byte_size(RESP_SIZE_LIMIT as u64)
            ),
        }
    }
}

pub async fn pull_feed(url: &str) -> Result<Rss, FeedError> {
    let mut resp = client().get(url).send().await?.error_for_status()?;
    if let Some(len) = resp.content_length() {
        if len > RESP_SIZE_LIMIT as u64 {
            return Err(FeedError::TooLarge);
        }
    }
    let mut buf = Vec::new(); // TODO: capacity?
    while let Some(bytes) = resp.chunk().await? {
        if buf.len() + bytes.len() > RESP_SIZE_LIMIT {
            return Err(FeedError::TooLarge);
        }
        buf.extend_from_slice(&bytes);
    }

    let feed = crate::feed::parse(std::io::Cursor::new(buf))?;
    Ok(crate::feed::fix_relative_url(feed, url))
}

fn client() -> Arc<reqwest::Client> {
    static mut CLIENT: Option<Arc<reqwest::Client>> = None;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        let mut headers = reqwest::header::HeaderMap::new();
        let ua = format!(
            concat!(
                env!("CARGO_PKG_NAME"),
                "/",
                env!("CARGO_PKG_VERSION"),
                " (+https://t.me/{})"
            ),
            crate::BOT_NAME.get().expect("BOT_NAME not initialized")
        );
        headers.insert(
            reqwest::header::USER_AGENT,
            reqwest::header::HeaderValue::from_str(&ua).unwrap(),
        );
        let mut client_builder = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .default_headers(headers)
            .redirect(reqwest::redirect::Policy::limited(5));

        if env::var("RSSBOT_DONT_PROXY_FEEDS")
            .or_else(|_| env::var("rssbot_dont_proxy_feeds"))
            .is_ok()
        {
            client_builder = client_builder.no_proxy();
        }

        let client = client_builder.build().unwrap();

        unsafe {
            CLIENT = Some(Arc::new(client));
        }
    });

    unsafe { CLIENT.clone() }.unwrap()
}

// TODO: const fn?
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
