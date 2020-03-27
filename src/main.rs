use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use anyhow::Context;
use once_cell::sync::OnceCell;
use reqwest;
use structopt::StructOpt;
use tbot;
use tokio;

mod data;
mod feed;

use crate::data::Database;

static BOT_NAME: OnceCell<String> = OnceCell::new();

#[derive(Debug, StructOpt)]
#[structopt(about = "A simple Telegram RSS bot.")]
struct Opt {
    /// Telegram bot token
    token: String,
    /// Path to database
    #[structopt(short = "d", default_value = "./rssbot.json")]
    database: PathBuf,
}

macro_rules! handle {
    ($env: expr, $f: expr) => {{
        let env = $env.clone();
        let f = $f;
        move |cmd| {
            let future = f(env.clone(), cmd);
            async {
                if let Err(e) = future.await {
                    dbg!(e);
                }
            }
        }
    }};
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opt = Opt::from_args();
    let db = Arc::new(Mutex::new(Database::open(opt.database)?));
    let bot = tbot::Bot::new(opt.token);
    let me = bot
        .get_me()
        .call()
        .await
        .context("Initialization failed, check your network and Telegram token")?;

    BOT_NAME.set(me.user.username.clone().unwrap()).unwrap();

    let mut event_loop = bot.event_loop();
    event_loop.username(me.user.username.unwrap());
    event_loop.command("rss", handle!(db, handlers::rss));
    event_loop.command("sub", handle!(db, handlers::sub));

    event_loop.polling().start().await.unwrap();
    Ok(())
}

mod handlers {
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::Mutex;
    use std::sync::{Arc, Once};
    use std::time::Duration;

    use reqwest;
    use tbot::{
        connectors::Https,
        contexts::{Command, Text},
        types::parameters,
    };

    use crate::data::{DataError, Database};
    use crate::feed::RSS;

    pub const TELEGRAM_MAX_MSG_LEN: usize = 4096;
    const RESP_SIZE_LIMIT: usize = 2 * 1024 * 1024;

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
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .default_headers(headers)
                .redirect(reqwest::redirect::Policy::limited(5))
                .build()
                .unwrap();

            unsafe {
                CLIENT = Some(Arc::new(client));
            }
        });

        unsafe { CLIENT.clone() }.unwrap()
    }

    pub async fn rss(
        db: Arc<Mutex<Database>>,
        cmd: Arc<Command<Text<Https>>>,
    ) -> anyhow::Result<()> {
        let chat = cmd.chat.id;
        let text = &cmd.text.value;

        let feeds = db.lock().unwrap().subscribed_feeds(chat.0);
        let msgs = if let Some(feeds) = feeds {
            format_and_split_msgs("订阅列表：".to_string(), &feeds, |feed| {
                format!(
                    "<a href=\"{}\">{}</a>",
                    Escape(&feed.link),
                    Escape(&feed.title)
                )
            })
        } else {
            vec!["订阅列表为空".to_string()]
        };

        let mut prev_msg = cmd.message_id;
        for msg in msgs {
            let text = parameters::Text::html(&msg);
            let msg = cmd.bot
                .send_message(chat, text)
                .reply_to_message_id(prev_msg)
                .call()
                .await?;
            prev_msg = msg.id;
        }
        Ok(())
    }

    pub async fn sub(
        db: Arc<Mutex<Database>>,
        cmd: Arc<Command<Text<Https>>>,
    ) -> anyhow::Result<()> {
        let chat_id = cmd.chat.id;
        let text = &cmd.text.value;
        let args = text.split_whitespace().collect::<Vec<_>>();
        let mut target_id = chat_id;
        let feed_url;

        match &*args {
            [url] => feed_url = url,
            [channel, url] => {
                //...
                target_id = todo!();
                feed_url = url;
            }
            [..] => {
                return Ok(());
            }
        };
        let msg = match pull_feed(feed_url).await {
            Ok(feed) => match db.lock().unwrap().subscribe(target_id.0, feed_url, &feed) {
                Ok(()) => format!("{} 订阅成功", feed.title),
                Err(DataError::Subscribed) => "".into(),
                Err(e) => unreachable!(e),
            },
            Err(e) => format!("订阅失败 {}", e),
        };
        cmd.bot
            .send_message(chat_id, text)
            .reply_to_message_id(cmd.message_id)
            .call()
            .await?;
        Ok(())
    }

    async fn pull_feed(url: &str) -> anyhow::Result<RSS> {
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

        crate::feed::parse(std::io::Cursor::new(buf))
    }

    pub fn format_and_split_msgs<T, F>(head: String, data: &[T], line_format_fn: F) -> Vec<String>
    where
        F: Fn(&T) -> String,
    {
        let mut msgs = vec![head];
        for item in data {
            let line = line_format_fn(item);
            if msgs.last_mut().unwrap().len() + line.len() > TELEGRAM_MAX_MSG_LEN {
                msgs.push(line);
            } else {
                let msg = msgs.last_mut().unwrap();
                msg.push('\n');
                msg.push_str(&line);
            }
        }
        msgs
    }

    pub struct Escape<'a>(pub &'a str);

    impl<'a> ::std::fmt::Display for Escape<'a> {
        fn fmt(&self, fmt: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
            // https://core.telegram.org/bots/api#html-style
            let Escape(s) = *self;
            let pile_o_bits = s;
            let mut last = 0;
            for (i, ch) in s.bytes().enumerate() {
                match ch as char {
                    '<' | '>' | '&' | '"' => {
                        fmt.write_str(&pile_o_bits[last..i])?;
                        let s = match ch as char {
                            '>' => "&gt;",
                            '<' => "&lt;",
                            '&' => "&amp;",
                            '"' => "&quot;",
                            _ => unreachable!(),
                        };
                        fmt.write_str(s)?;
                        last = i + 1;
                    }
                    _ => {}
                }
            }

            if last < s.len() {
                fmt.write_str(&pile_o_bits[last..])?;
            }
            Ok(())
        }
    }
}
