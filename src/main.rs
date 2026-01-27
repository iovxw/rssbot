#![feature(error_reporter)]
#![recursion_limit = "256"]

use std::convert::TryInto;
use std::env;
use std::panic;
use std::path::PathBuf;
use std::process;
use std::sync::Arc;

use anyhow::{anyhow, Context};
use hyper_proxy::{Intercept, Proxy};
use std::sync::OnceLock;
use structopt::StructOpt;
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

static BOT_NAME: OnceLock<String> = OnceLock::new();
static BOT_ID: OnceLock<tbot::types::user::Id> = OnceLock::new();

#[derive(Debug, StructOpt)]
#[structopt(
    about = "A simple Telegram RSS bot.",
    after_help = "NOTE: You can get <user id> using bots like @userinfobot @getidsbot"
)]
pub struct Opt {
    /// Telegram bot token
    #[structopt(
        env = "RSSBOT_TOKEN",
        hide_env_values = true
    )]
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
    #[structopt(long, value_name = "bytes", default_value = "2M")]
    max_feed_size: String,
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

/// Parse human readable size into bytes.
fn parse_human_size(s: &str) -> anyhow::Result<u64> {
    const BASE: u64 = 1024;
    let s = s.trim().trim_end_matches(|x| x == 'B' || x == 'b');
    match s.chars().last().map(|x| x.to_ascii_lowercase()) {
        Some('b') => Ok(s[..s.len() - 1].parse()?),
        Some('k') => Ok(s[..s.len() - 1].parse::<u64>()? * BASE),
        Some('m') => Ok(s[..s.len() - 1].parse::<u64>()? * BASE.pow(2)),
        Some('g') => Ok(s[..s.len() - 1].parse::<u64>()? * BASE.pow(3)),
        Some('t') => Ok(s[..s.len() - 1].parse::<u64>()? * BASE.pow(4)),
        Some(x) if x.is_ascii_digit() => Ok(s.parse()?),
        Some(x) => Err(anyhow!("invalid size character: {}", x)),
        None => Err(anyhow!("empty size")),
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    enable_fail_fast();

    let opt = Opt::from_args();
    let db = Arc::new(Mutex::new(Database::open(opt.database.clone())?));
    let bot_builder =
        tbot::bot::Builder::with_string_token(opt.token.clone()).server_uri(opt.api_uri.clone());
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
    crate::client::init_client(
        &bot_name,
        opt.insecure,
        parse_human_size(&opt.max_feed_size).context("Invalid max_feed_size")?,
    );

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_human_size() {
        assert_eq!(parse_human_size("2M").unwrap(), 2_097_152);
        assert_eq!(parse_human_size("2G").unwrap(), 2_147_483_648);
        assert_eq!(parse_human_size("2mb").unwrap(), 2_097_152);
        assert_eq!(parse_human_size("2097152").unwrap(), 2_097_152);
    }
}
