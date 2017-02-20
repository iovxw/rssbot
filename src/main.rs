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
use std::rc::Rc;
use std::cell::RefCell;

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

    let db = Rc::new(RefCell::new(data::Database::open(datafile)
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
        .unwrap()));

    env_logger::init().unwrap();

    let mut lp = Core::new().unwrap();
    let bot = telebot::RcBot::new(lp.handle(), token).update_interval(200);

    {
        let db = db.clone();
        let handle = bot.new_cmd("/rss")
            .and_then(move |(bot, msg)| {
                let text = msg.text.unwrap();
                let args: Vec<&str> = text.split_whitespace().collect();
                let raw: bool;
                let subscriber: i64;
                match args.len() {
                    0 => {
                        raw = false;
                        subscriber = msg.chat.id;
                    }
                    1 => {
                        if args[0] == "raw" {
                            raw = true;
                            subscriber = msg.chat.id;
                        } else {
                            raw = false;
                            let channel = args[0];
                            subscriber = msg.chat.id;
                        }
                    }
                    2 => {
                        raw = true;
                        let channel = args[0];
                        subscriber = msg.chat.id;
                    }
                    _ => {
                        return bot.message(msg.chat.id,
                                     "使用方法： /rss <Channel ID> <raw>".to_string())
                            .send()
                    }
                }

                match db.borrow().get_subscribed_feeds(subscriber) {
                    Some(feeds) => {
                        let mut text = String::from("订阅列表:");
                        if raw {
                            for feed in feeds {
                                text.push_str(&format!("\n<a href=\"{}\">{}</a>",
                                                       feed.title,
                                                       feed.link));
                            }
                            bot.message(msg.chat.id, text)
                                .parse_mode("HTML")
                                .disable_web_page_preview(true)
                                .send()
                        } else {
                            for feed in feeds {
                                text.push_str(&format!("\n{}: {}", feed.title, feed.link));
                            }
                            bot.message(msg.chat.id, text)
                                .disable_web_page_preview(true)
                                .send()
                        }
                    }
                    None => bot.message(msg.chat.id, "订阅列表为空".to_string()).send(),
                }
            });
        bot.register(handle);
    }
    {
        let db = db.clone();
        let handle = bot.new_cmd("/sub")
            .and_then(move |(bot, msg)| {
                let text = msg.text.unwrap();
                let args: Vec<&str> = text.split_whitespace().collect();
                let feed_link: &str;
                let subscriber: i64;
                match args.len() {
                    1 => {
                        feed_link = args[0];
                        subscriber = msg.chat.id;
                    }
                    2 => {
                        let channel = args[0];
                        subscriber = msg.chat.id;
                        feed_link = args[1];
                    }
                    _ => {
                        return bot.message(msg.chat.id,
                                     "使用方法： /sub [Channel ID] <RSS URL>".to_string())
                            .send()
                    }
                }

                match db.borrow_mut().subscribe(subscriber, feed_link, &rss::Channel::default()) {
                    Ok(_) => bot.message(msg.chat.id, "订阅成功".to_string()).send(),
                    Err(errors::Error(errors::ErrorKind::AlreadySubscribed, _)) => {
                        bot.message(msg.chat.id, "已订阅过的 RSS".to_string()).send()
                    }
                    Err(e) => {
                        log_error(&e);
                        bot.message(msg.chat.id, format!("error: {}", e)).send()
                    }
                }
            });
        bot.register(handle);
    }
    {
        let db = db.clone();
        let handle = bot.new_cmd("/unsub")
            .and_then(move |(bot, msg)| {
                let text = msg.text.unwrap();
                let args: Vec<&str> = text.split_whitespace().collect();
                let feed_link: &str;
                let subscriber: i64;
                match args.len() {
                    1 => {
                        feed_link = args[0];
                        subscriber = msg.chat.id;
                    }
                    2 => {
                        let channel = args[0];
                        subscriber = msg.chat.id;
                        feed_link = args[1];
                    }
                    _ => {
                        return bot.message(msg.chat.id,
                                     "使用方法： /unsub [Channel ID] <RSS URL>".to_string())
                            .send()
                    }
                }

                match db.borrow_mut().unsubscribe(subscriber, feed_link) {
                    Ok(_) => bot.message(msg.chat.id, "退订成功".to_string()).send(),
                    Err(errors::Error(errors::ErrorKind::NotSubscribed, _)) => {
                        bot.message(msg.chat.id, "未订阅过的 Feed".to_string()).send()
                    }
                    Err(e) => {
                        log_error(&e);
                        bot.message(msg.chat.id, format!("error: {}", e)).send()
                    }
                }
            });
        bot.register(handle);
    }

    loop {
        if let Err(err) = bot.run(&mut lp) {
            error!("{:?}", err);
        }
    }
}
