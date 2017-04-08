#![feature(conservative_impl_trait)]

#[macro_use]
extern crate log;
extern crate env_logger;
#[macro_use]
extern crate error_chain;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
extern crate quick_xml;
extern crate curl;
extern crate futures;
extern crate tokio_core;
extern crate tokio_curl;
extern crate telebot;
#[macro_use]
extern crate lazy_static;
extern crate regex;
extern crate pinyin_order;

use std::io::prelude::*;

use tokio_core::reactor::Core;
use futures::Stream;

mod errors;
mod feed;
mod data;
mod utlis;
mod cmdhandels;
mod fetcher;
mod checker;

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

    let db = data::Database::open(datafile)
        .map_err(|e| {
            writeln!(&mut std::io::stderr(), "error: {}", e).unwrap();
            for e in e.iter().skip(1) {
                writeln!(&mut std::io::stderr(), "caused by: {}", e).unwrap();
            }
            if let Some(backtrace) = e.backtrace() {
                writeln!(&mut std::io::stderr(), "backtrace: {:?}", backtrace).unwrap();
            }
            std::process::exit(1);
        })
        .unwrap();

    env_logger::init().unwrap();

    let mut lp = Core::new().unwrap();
    let lphandle = lp.handle();
    let bot = lp.run(telebot::RcBot::new(lphandle, token))
        .expect("failed to initialize bot")
        .update_interval(200);

    cmdhandels::register_commands(&bot, &db, lp.handle());

    fetcher::spawn_fetcher(bot.clone(), db.clone(), lp.handle());

    checker::spawn_subscriber_alive_checker(bot.clone(), db, lp.handle());

    loop {
        if let Err(err) = lp.run(bot.get_stream().for_each(|_| Ok(()))) {
            error!("telebot: {:?}", err);
        }
    }
}
