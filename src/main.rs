#![feature(backtrace)]
#![recursion_limit = "256"]

use std::convert::TryInto;
use std::env;
use std::panic;
use std::path::PathBuf;
use std::process;
use std::sync::{Arc, Mutex}; // TODO: async Mutex

use anyhow::Context;
use once_cell::sync::OnceCell;
use structopt::StructOpt;
use tbot::{
    self,
    proxy::{Intercept, Proxy},
};
use tokio;

// Include the tr! macro and localizations
include!(concat!(env!("OUT_DIR"), "/ctl10n_macros.rs"));

mod client;
mod commands;
mod data;
mod feed;
mod fetcher;
mod gardener;
mod messages;
mod opml;

use crate::data::Database;

static BOT_NAME: OnceCell<String> = OnceCell::new();
static BOT_ID: OnceCell<tbot::types::user::Id> = OnceCell::new();

#[derive(Debug, StructOpt)]
#[structopt(
    about = "A simple Telegram RSS bot.",
    after_help = "NOTE: You can get <user id> using bots like @userinfobot @getidsbot"
)]
pub struct Opt {
    /// Telegram bot token
    token: String,
    /// Path to database
    #[structopt(
        short = "d",
        long,
        value_name = "path",
        default_value = "./rssbot.json"
    )]
    database: PathBuf,
    /// Minimum fetch interval
    #[structopt(
        long,
        value_name = "seconds",
        default_value = "300",
        validator(check_interval)
    )]
    // default is 5 minutes
    min_interval: u32,
    /// Maximum fetch interval
    #[structopt(
        long,
        value_name = "seconds",
        default_value = "43200",
        validator(check_interval)
    )]
    // default is 12 hours
    max_interval: u32,
    /// Maximum feed size, 0 is unlimited
    #[structopt(long, value_name = "bytes", default_value = "2097152")]
    // default is 2MiB
    max_feed_size: u64,
    /// Single user mode, only specified user can use this bot
    #[structopt(long, value_name = "user id")]
    single_user: Option<i64>,
    /// Multi-user mode, only specified users can use this bot, split users with `,'
    #[structopt(long, value_name = "users id")]
    admins: Option<String>,
    /// Make bot commands only accessible for group admins.
    #[structopt(long)]
    restricted: bool,
    /// DANGER: Insecure mode, accept invalid TLS certificates
    #[structopt(long)]
    insecure: bool,
}

fn check_interval(s: String) -> Result<(), String> {
    s.parse::<u32>().map_err(|e| e.to_string()).and_then(|r| {
        if r < 1 {
            Err("must >= 1".into())
        } else {
            Ok(())
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
                    print_error(e);
                }
            }
        }
    }};
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    enable_fail_fast();

    let opt = Opt::from_args();
    let db = Arc::new(Mutex::new(Database::open(opt.database.clone())?));
    let bot = if let Some(proxy) = init_proxy() {
        tbot::Bot::with_proxy(opt.token.clone(), proxy)
    } else {
        tbot::Bot::new(opt.token.clone())
    };
    let me = bot
        .get_me()
        .call()
        .await
        .context("Initialization failed, check your network and Telegram token")?;

    let bot_name = me.user.username.clone().unwrap();
    crate::client::init_client(&bot_name, opt.insecure, opt.max_feed_size);

    BOT_NAME.set(bot_name).unwrap();
    BOT_ID.set(me.user.id).unwrap();

    gardener::start_pruning(bot.clone(), db.clone());
    fetcher::start(bot.clone(), db.clone(), opt.min_interval, opt.max_interval);

    let opt = Arc::new(opt);
    let check_command = move |cmd| {
        let opt = opt.clone();
        async move { commands::check_command(&opt, cmd).await }
    };

    let mut event_loop = bot.event_loop();
    event_loop.username(me.user.username.unwrap());
    event_loop.start_if(check_command.clone(), handle!(db, commands::start));
    event_loop.command_if("rss", check_command.clone(), handle!(db, commands::rss));
    event_loop.command_if("sub", check_command.clone(), handle!(db, commands::sub));
    event_loop.command_if("unsub", check_command.clone(), handle!(db, commands::unsub));
    event_loop.command_if(
        "export",
        check_command.clone(),
        handle!(db, commands::export),
    );

    event_loop.polling().start().await.unwrap();
    Ok(())
}

// Exit the process when any worker thread panicked
fn enable_fail_fast() {
    let default_panic_hook = panic::take_hook();
    panic::set_hook(Box::new(move |e| {
        default_panic_hook(e);
        process::exit(101);
    }));
}

fn init_proxy() -> Option<Proxy> {
    // Telegram Bot API only uses https, no need to check http_proxy
    env::var("HTTPS_PROXY")
        .or_else(|_| env::var("https_proxy"))
        .map(|uri| {
            let uri = uri
                .try_into()
                .unwrap_or_else(|e| panic!("Illegal HTTPS_PROXY: {}", e));
            Proxy::new(Intercept::All, uri)
        })
        .ok()
}

fn print_error<E: std::error::Error>(err: E) {
    eprintln!("Error: {}", err);
    let mut deepest_backtrace = err.backtrace();

    let mut err: &dyn std::error::Error = &err;
    if let Some(e) = err.source() {
        eprintln!("\nCaused by:");
        let multiple = e.source().is_some();
        let mut line_counter = 0..;
        while let (Some(e), Some(line)) = (err.source(), line_counter.next()) {
            if multiple {
                eprint!("{: >4}: ", line)
            } else {
                eprint!("    ")
            };
            eprintln!("{}", e);
            if let Some(backtrace) = e.backtrace() {
                deepest_backtrace = Some(backtrace);
            }
            err = e;
        }
    }

    if let Some(backtrace) = deepest_backtrace {
        eprintln!("\nBacktrace:\n{}", backtrace);
    }
}
