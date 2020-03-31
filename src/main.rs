#![feature(backtrace)]

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
mod handlers;

use crate::data::Database;

static BOT_NAME: OnceCell<String> = OnceCell::new();
static BOT_ID: OnceCell<tbot::types::user::Id> = OnceCell::new();

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
    let bot = tbot::Bot::new(opt.token);
    let me = bot
        .get_me()
        .call()
        .await
        .context("Initialization failed, check your network and Telegram token")?;

    BOT_NAME.set(me.user.username.clone().unwrap()).unwrap();
    BOT_ID.set(me.user.id).unwrap();

    let mut event_loop = bot.event_loop();
    event_loop.username(me.user.username.unwrap());
    event_loop.command("rss", handle!(db, handlers::rss));
    event_loop.command("sub", handle!(db, handlers::sub));
    event_loop.command("unsub", handle!(db, handlers::unsub));
    event_loop.command("export", handle!(db, handlers::export));

    event_loop.polling().start().await.unwrap();
    Ok(())
}
