#![feature(error_reporter)]
#![recursion_limit = "256"]

use std::env;
use std::panic;
use std::path::PathBuf;
use std::process;
use std::sync::Arc;

use anyhow::{anyhow, Context};
use reqwest::Url;
use std::sync::OnceLock;
use structopt::StructOpt;
use teloxide::{prelude::*, types::UserId};
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

static BOT_ID: OnceLock<UserId> = OnceLock::new();

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
    api_uri: Url,
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
    let bot = build_bot(&opt)?;
    let me = bot
        .get_me()
        .await
        .context("Initialization failed, check your network and Telegram token")?;

    let bot_name = me.user.username.clone().unwrap();
    crate::client::init_client(
        &bot_name,
        opt.insecure,
        parse_human_size(&opt.max_feed_size).context("Invalid max_feed_size")?,
    );

    // Cache the bot identity once so command checks and background jobs don't
    // need to call get_me again.
    BOT_ID.set(me.user.id).unwrap();

    gardener::start_pruning(bot.clone(), db.clone());
    fetcher::start(bot.clone(), db.clone(), opt.min_interval, opt.max_interval);

    let opt = Arc::new(opt);

    let handler = dptree::entry()
        .branch(Update::filter_message().filter_command::<commands::BotCommand>().endpoint(
            |bot: Bot,
             msg: Message,
             cmd: commands::BotCommand,
             opt: Arc<crate::Opt>,
             db: Arc<Mutex<Database>>| async move {
                commands::handle_message(bot, msg, cmd, opt, db).await;
                respond(())
            },
        ))
        .branch(
            Update::filter_channel_post()
                .filter_command::<commands::BotCommand>()
                .endpoint(
                    |bot: Bot,
                     msg: Message,
                     cmd: commands::BotCommand,
                     opt: Arc<crate::Opt>,
                     db: Arc<Mutex<Database>>| async move {
                        commands::handle_message(bot, msg, cmd, opt, db).await;
                        respond(())
                    },
                ),
        )
        ;

    Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![opt, db])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;

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

fn init_proxy() -> Option<String> {
    // Telegram Bot API only uses https, no need to check http_proxy
    env::var("HTTPS_PROXY")
        .or_else(|_| env::var("https_proxy"))
        .ok()
}

fn build_bot(opt: &Opt) -> anyhow::Result<Bot> {
    let mut client_builder = teloxide::net::default_reqwest_settings();

    if let Some(proxy) = init_proxy() {
        client_builder = client_builder.proxy(
            reqwest::Proxy::all(&proxy).context("Illegal HTTPS_PROXY")?,
        );
    }

    let client = client_builder
        .build()
        .context("Failed to build Telegram API client")?;

    Ok(Bot::with_client(opt.token.clone(), client).set_api_url(opt.api_uri.clone()))
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

    #[test]
    fn test_parse_human_size_rejects_invalid_input() {
        assert_eq!(parse_human_size("2TB").unwrap(), 2 * 1024_u64.pow(4));
        assert!(parse_human_size("2P").is_err());
        assert!(parse_human_size("").is_err());
    }

    #[test]
    fn test_check_interval() {
        assert!(check_interval("1".to_owned()).is_ok());
        assert!(check_interval("600".to_owned()).is_ok());
        assert!(check_interval("0".to_owned()).is_err());
        assert!(check_interval("not-a-number".to_owned()).is_err());
    }
}
