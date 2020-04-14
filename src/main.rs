#![feature(backtrace)]
#![recursion_limit = "256"]

use std::convert::TryInto;
use std::env;
use std::path::PathBuf;
use std::sync::{Arc, Mutex}; // TODO: async Mutex

use anyhow::Context;
use hyper_proxy::{Intercept, Proxy, ProxyConnector};
use once_cell::sync::OnceCell;
use structopt::StructOpt;
use tbot;
use tokio;

mod client;
mod data;
mod feed;
mod fetcher;
mod gardener;
mod handlers;
mod messages;

use crate::data::Database;

static BOT_NAME: OnceCell<String> = OnceCell::new();
static BOT_ID: OnceCell<tbot::types::user::Id> = OnceCell::new();

#[derive(Debug, StructOpt)]
#[structopt(about = "A simple Telegram RSS bot.")]
struct Opt {
    /// Telegram bot token
    token: String,
    /// Path to database
    #[structopt(short = "d", long, default_value = "./rssbot.json")]
    database: PathBuf,

    #[structopt(long, default_value = "300", parse(try_from_str = parse_interval))] // 5 minutes
    min_interval: u32,

    #[structopt(long, default_value = "43200", parse(try_from_str = parse_interval))] // 12 hours
    max_interval: u32,
}

fn parse_interval(s: &str) -> Result<u32, String> {
    s.parse::<u32>().map_err(|e| e.to_string()).and_then(|r| {
        if r < 1 {
            Err("must >= 1".into())
        } else {
            Ok(r)
        }
    })
}

macro_rules! handle {
    ($env: expr, $f: expr) => {{
        let env = $env.clone();
        let f = $f;
        move |cmd| {
            let future = f(env.clone(), cmd);
            async {
                if let Err(e) = future.await {
                    dbg!(&e);
                    dbg!(e.backtrace());
                }
            }
        }
    }};
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opt = Opt::from_args();
    let db = Arc::new(Mutex::new(Database::open(opt.database)?));
    let connector = init_bot_connector();
    let bot = tbot::Bot::with_connector(opt.token, connector);
    let me = bot
        .get_me()
        .call()
        .await
        .context("Initialization failed, check your network and Telegram token")?;

    BOT_NAME.set(me.user.username.clone().unwrap()).unwrap();
    BOT_ID.set(me.user.id).unwrap();

    gardener::start_pruning(bot.clone(), db.clone());
    fetcher::start(bot.clone(), db.clone(), opt.min_interval, opt.max_interval);
    let mut event_loop = bot.event_loop();
    event_loop.username(me.user.username.unwrap());
    event_loop.start(handle!(db, handlers::start));
    event_loop.command("rss", handle!(db, handlers::rss));
    event_loop.command("sub", handle!(db, handlers::sub));
    event_loop.command("unsub", handle!(db, handlers::unsub));
    event_loop.command("export", handle!(db, handlers::export));

    event_loop.polling().start().await.unwrap();
    Ok(())
}

fn init_bot_connector() -> ProxyConnector<tbot::connectors::Https> {
    // Telegram Bot API only uses https, no need to check http_proxy
    let proxy = env::var("HTTPS_PROXY").or_else(|_| env::var("https_proxy"));
    let mut c = ProxyConnector::new(tbot::connectors::https())
        .unwrap_or_else(|e| panic!("Failed to construct a proxy connector: {}", e));
    if let Ok(ref proxy) = proxy {
        dbg!(proxy);
        let uri = proxy
            .try_into()
            .unwrap_or_else(|e| panic!("Illegal HTTPS_PROXY: {}", e));
        let proxy = Proxy::new(Intercept::All, uri);
        c.add_proxy(proxy);
    }
    c
}
