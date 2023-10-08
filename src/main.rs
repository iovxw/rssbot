#![feature(error_reporter)]
#![recursion_limit = "256"]

use std::convert::TryInto;
use std::env;
use std::panic;
use std::path::PathBuf;
use std::process;
use std::sync::Arc;

use anyhow::Context;
use hyper_proxy::{Intercept, Proxy};
use once_cell::sync::OnceCell;
use structopt::StructOpt;
use tbot;
use tbot::bot::Uri;
use tokio::{self, sync::Mutex};

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
    /// Private mode, only specified user can use this bot.
    /// This argument can be passed multiple times to allow multiple admins
    #[structopt(
        long,
        value_name = "user id",
        number_of_values = 1,
        alias = "single_user" // For compatibility
    )]
    admin: Vec<i64>,
    /// Make bot commands only accessible for group admins.
    #[structopt(long)]
    restricted: bool,
    /// Custom telegram api URI
    #[structopt(
        long,
        value_name = "tgapi-uri",
        default_value = "https://api.telegram.org/"
    )]
    api_uri: Uri,
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    enable_fail_fast();

    let opt = Opt::from_args();
    let db = Arc::new(Mutex::new(Database::open(opt.database.clone())?));
    let bot_builder = tbot::bot::Builder::with_string_token(opt.token.clone())
        .server_uri(opt.api_uri.clone());
    let bot = if let Some(proxy) = init_proxy() {
        bot_builder.proxy(proxy).build()
    } else {
        bot_builder.build()
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

    let mut event_loop = bot.event_loop();
    event_loop.username(me.user.username.unwrap());
    commands::register_commands(&mut event_loop, opt, db);

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
    eprintln!(
        "Error: {}",
        std::error::Report::new(err)
            .pretty(true)
            .show_backtrace(true)
    );
}
