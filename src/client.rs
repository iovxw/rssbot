use std::env;
use std::sync::{Arc, Once};
use std::time::Duration;

use reqwest;

use crate::feed::Rss;

const RESP_SIZE_LIMIT: usize = 2 * 1024 * 1024;

pub async fn pull_feed(url: &str) -> anyhow::Result<Rss> {
    let mut resp = client().get(url).send().await?.error_for_status()?;
    if let Some(len) = resp.content_length() {
        if len > RESP_SIZE_LIMIT as u64 {
            return Err(anyhow::format_err!("too big"));
        }
    }
    let mut buf = Vec::new(); // TODO: capacity?
    while let Some(bytes) = resp.chunk().await? {
        if buf.len() + bytes.len() > RESP_SIZE_LIMIT {
            return Err(anyhow::format_err!("too big"));
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
