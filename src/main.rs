#[macro_use]
extern crate log;
extern crate env_logger;
#[macro_use]
extern crate error_chain;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
extern crate quick_xml;
extern crate rss;
extern crate atom_syndication as atom;
extern crate curl;
extern crate futures;
extern crate tokio_core;
extern crate tokio_curl;
extern crate telebot;

use std::io::prelude::*;

use telebot::functions::*;
use tokio_core::reactor::Core;
use futures::Future;
use futures::Stream;

mod errors;
mod feed;
mod data;

fn log_error(e: &errors::Error) {
    warn!("error: {}", e);
    for e in e.iter().skip(1) {
        warn!("caused by: {}", e);
    }
    if let Some(backtrace) = e.backtrace() {
        warn!("backtrace: {:?}", backtrace);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        writeln!(&mut std::io::stderr(),
                 "Usage: {} DATAFILE TELEGRAM-BOT-TOKEN",
                 args[0])
            .unwrap();
        std::process::exit(1);
    }
    let datafile = &args[1];
    let token = &args[2];

    let mut db = data::Database::open(datafile)
        .map_err(|e| {
            writeln!(&mut std::io::stderr(), "error: {}", e).unwrap();
            for e in e.iter().skip(1) {
                writeln!(&mut std::io::stderr(), "caused by: {}", e).unwrap();
            }
            if let Some(backtrace) = e.backtrace() {
                writeln!(&mut std::io::stderr(), "backtrace: {:?}", backtrace).unwrap();
            }
            ::std::process::exit(1);
        })
        .unwrap();

    env_logger::init().unwrap();

    let mut lp = Core::new().unwrap();
    let bot = telebot::RcBot::new(lp.handle(), token).update_interval(200);

    let handle = bot.new_cmd("/sub")
        .and_then(move |(bot, msg)| {
            let mut text = msg.text.unwrap().to_owned();
            if text.is_empty() {
                text = "<empty>".into();
            }

            match db.subscribe(msg.chat.id, &text, &rss::Channel::default()) {
                Ok(_) => bot.message(msg.chat.id, text).send(),
                Err(errors::Error(errors::ErrorKind::AlreadySubscribed, _)) => {
                    bot.message(msg.chat.id, "已订阅过的 Feed".to_string()).send()
                }
                Err(e) => {
                    log_error(&e);
                    bot.message(msg.chat.id, format!("error: {}", e)).send()
                }
            }
        });

    bot.register(handle);

    loop {
        if let Err(err) = bot.run(&mut lp) {
            error!("{:?}", err);
        }
    }
}
