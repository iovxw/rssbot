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

    let handle = bot.new_cmd("/reply")
        .and_then(|(bot, msg)| {
            let mut text = msg.text.unwrap().clone();
            if text.is_empty() {
                text = "<empty>".into();
            }

            bot.message(msg.chat.id, text).send()
        });

    bot.register(handle);

    loop {
        if let Err(err) = bot.run(&mut lp) {
            error!("{:?}", err);
        }
    }
}
