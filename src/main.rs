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
mod telebot_missing;

use telebot_missing::{get_chat_string, edit_message_text};

fn log_error(e: &errors::Error) {
    warn!("error: {}", e);
    for e in e.iter().skip(1) {
        warn!("caused by: {}", e);
    }
    if let Some(backtrace) = e.backtrace() {
        warn!("backtrace: {:?}", backtrace);
    }
}

fn check_channel<'a>(bot: &telebot::RcBot,
                     channel: &str,
                     chat_id: i64,
                     user_id: i64)
                     -> impl Future<Item = Option<i64>, Error = telebot::Error> + 'a {
    let channel = channel.to_owned();
    let bot = bot.to_owned();
    bot.message(chat_id, "正在验证 Channel".to_string())
        .send()
        .map_err(|e| Some(e))
        .and_then(move |(bot, msg)| {
            let msg_id = msg.message_id;
            get_chat_string(&bot, channel)
                .send()
                .or_else(move |e| -> Box<Future<Item = _, Error = Option<telebot::Error>>> {
                    match e {
                        telebot::Error::Telegram(err_msg) => {
                            Box::new(edit_message_text(&bot,
                                                       chat_id,
                                                       msg_id,
                                                       format!("无法找到目标 Channel: {}", err_msg))
                                .send()
                                .then(|result| match result {
                                    Ok(_) => futures::future::err(None),
                                    Err(e) => futures::future::err(Some(e)),
                                }))
                        }
                        _ => Box::new(futures::future::err(Some(e))),
                    }
                })
                .map(move |(bot, channel)| (bot, channel, msg_id))
        })
        .and_then(move |(bot, channel, msg_id)| -> Box<Future<Item = _, Error = Option<_>>> {
            if channel.kind != "channel" {
                Box::new(bot.message(chat_id, "目标需为 Channel".to_string())
                    .send()
                    .then(|result| match result {
                        Ok(_) => Err(None),
                        Err(e) => Err(Some(e)),
                    }))
            } else {
                let channel_id = channel.id;
                Box::new(bot.unban_chat_administrators(channel_id)
                    .send()
                    .or_else(move |e| -> Box<Future<Item = _, Error = Option<_>>> {
                        match e {
                            telebot::Error::Telegram(error_msg) => {
                                Box::new(edit_message_text(&bot,
                                                           chat_id,
                                                           msg_id,
                                                           format!("请先将本 Bot 加入目标 Channel 并设为管理员: {}",
                                                                   error_msg))
                                    .send()
                                    .then(|result| match result {
                                        Ok(_) => futures::future::err(None),
                                        Err(e) => futures::future::err(Some(e)),
                                    }))
                            }
                            _ => Box::new(futures::future::err(Some(e))),
                        }
                    })
                    .map(move |(bot, admins)| {
                        let admin_id_list = admins.iter().map(|member| member.user.id).collect::<Vec<i64>>();
                        (bot, admin_id_list, msg_id, channel_id)
                    }))
            }
        })
        .and_then(move |(bot, admin_id_list, msg_id, channel_id)| {
            bot.get_me()
                .send()
                .map_err(|e| Some(e))
                .map(move |(bot, me)| (bot, me.id, admin_id_list, msg_id, channel_id))
        })
        .and_then(move |(bot, bot_id, admin_id_list, msg_id, channel_id)| -> Box<Future<Item = i64, Error = Option<_>>> {
            if admin_id_list.contains(&bot_id) {
                if admin_id_list.contains(&user_id) {
                    Box::new(futures::future::ok(channel_id))
                } else {
                    Box::new(edit_message_text(&bot,
                                               chat_id,
                                               msg_id,
                                               "该命令只能由 Channel 管理员使用".to_string())
                        .send()
                        .then(|result| match result {
                            Ok(_) => futures::future::err(None),
                            Err(e) => futures::future::err(Some(e)),
                        }))
                }
            } else {
                Box::new(edit_message_text(&bot,
                                           chat_id,
                                           msg_id,
                                           "请将本 Bot 设为管理员".to_string())
                    .send()
                    .then(|result| match result {
                        Ok(_) => futures::future::err(None),
                        Err(e) => futures::future::err(Some(e)),
                    }))
            }
        })
        .then(|result| match result {
            Err(None) => futures::future::ok(None),
            Err(Some(e)) => futures::future::err(e),
            Ok(ok) => futures::future::ok(Some(ok)),
        })
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
            .map_err(|e| Some(e))
            .and_then(move |(bot, msg)| -> Box<Future<Item = _, Error = Option<_>>> {
                let text = msg.text.unwrap();
                let args: Vec<&str> = text.split_whitespace().collect();
                let raw: bool;
                let subscriber: Box<Future<Item = Option<i64>, Error = telebot::Error>>;
                match args.len() {
                    0 => {
                        raw = false;
                        subscriber = futures::future::ok(Some(msg.chat.id)).boxed();
                    }
                    1 => {
                        if args[0] == "raw" {
                            raw = true;
                            subscriber = futures::future::ok(Some(msg.chat.id)).boxed();
                        } else {
                            raw = false;
                            let channel = args[0];
                            subscriber = Box::new(check_channel(&bot, channel, msg.chat.id, msg.from.unwrap().id));
                        }
                    }
                    2 => {
                        raw = true;
                        let channel = args[0];
                        subscriber = Box::new(check_channel(&bot, channel, msg.chat.id, msg.from.unwrap().id));
                    }
                    _ => {
                        return Box::new(bot.message(msg.chat.id,
                                     "使用方法： /rss <Channel ID> <raw>".to_string())
                            .send()
                            .then(|result| match result {
                                Ok(_) => Err(None),
                                Err(e) => Err(Some(e)),
                            }))
                    }
                }

                let bot = bot.clone();
                let db = db.clone();
                let chat_id = msg.chat.id;
                Box::new(subscriber.then(|result| match result {
                        Ok(Some(ok)) => Ok(ok),
                        Ok(None) => Err(None),
                        Err(err) => Err(Some(err)),
                    })
                    .map(move |subscriber| (bot, db, subscriber, raw, chat_id)))
            })
            .and_then(move |(bot, db, subscriber, raw, chat_id)| {
                let r = match db.borrow().get_subscribed_feeds(subscriber) {
                    Some(feeds) => {
                        let mut text = String::from("订阅列表:");
                        if !raw {
                            for feed in feeds {
                                text.push_str(&format!("\n<a href=\"{}\">{}</a>", feed.title, feed.link));
                            }
                            bot.message(chat_id, text)
                                .parse_mode("HTML")
                                .disable_web_page_preview(true)
                                .send()
                        } else {
                            for feed in feeds {
                                text.push_str(&format!("\n{}: {}", feed.title, feed.link));
                            }
                            bot.message(chat_id, text)
                                .disable_web_page_preview(true)
                                .send()
                        }
                    }
                    None => {
                        bot.message(chat_id, "订阅列表为空".to_string())
                            .send()
                    }
                };
                r.map_err(|e| Some(e))
            })
            .then(|result| match result {
                Ok(_) => Ok(()),
                Err(None) => Ok(()),
                Err(Some(err)) => Err(err),
            });

        bot.register(handle);
    }
    {
        let db = db.clone();
        let handle = bot.new_cmd("/sub")
            .map_err(|e| Some(e))
            .and_then(move |(bot, msg)| -> Box<Future<Item = _, Error = Option<_>>> {
                let text = msg.text.unwrap();
                let args: Vec<&str> = text.split_whitespace().collect();
                let feed_link: &str;
                let subscriber: Box<Future<Item = Option<i64>, Error = telebot::Error>>;
                match args.len() {
                    1 => {
                        feed_link = args[0];
                        subscriber = futures::future::ok(Some(msg.chat.id)).boxed();
                    }
                    2 => {
                        let channel = args[0];
                        subscriber = Box::new(check_channel(&bot, channel, msg.chat.id, msg.from.unwrap().id));
                        feed_link = args[1];
                    }
                    _ => {
                        return Box::new(bot.message(msg.chat.id,
                                     "使用方法： /sub [Channel ID] <RSS URL>".to_string())
                            .send()
                            .then(|result| match result {
                                Ok(_) => Err(None),
                                Err(e) => Err(Some(e)),
                            }))
                    }
                }


                let bot = bot.clone();
                let db = db.clone();
                let feed_link = feed_link.to_owned();
                let chat_id = msg.chat.id;
                Box::new(subscriber.then(|result| match result {
                        Ok(Some(ok)) => Ok(ok),
                        Ok(None) => Err(None),
                        Err(err) => Err(Some(err)),
                    })
                    .map(move |subscriber| (bot, db, subscriber, feed_link, chat_id)))
            })
            .and_then(move |(bot, db, subscriber, feed_link, chat_id)| {
                let r = match db.borrow_mut().subscribe(subscriber, &feed_link, &rss::Channel::default()) {
                    Ok(_) => bot.message(chat_id, "订阅成功".to_string()).send(),
                    Err(errors::Error(errors::ErrorKind::AlreadySubscribed, _)) => bot.message(chat_id, "已订阅过的 RSS".to_string()).send(),
                    Err(e) => {
                        log_error(&e);
                        bot.message(chat_id, format!("error: {}", e)).send()
                    }
                };
                r.map_err(|e| Some(e))
            })
            .then(|result| match result {
                Ok(_) => Ok(()),
                Err(None) => Ok(()),
                Err(Some(err)) => Err(err),
            });

        bot.register(handle);
    }
    {
        let db = db.clone();
        let handle = bot.new_cmd("/unsub")
            .map_err(|e| Some(e))
            .and_then(move |(bot, msg)| -> Box<Future<Item = _, Error = Option<_>>> {
                let text = msg.text.unwrap();
                let args: Vec<&str> = text.split_whitespace().collect();
                let feed_link: &str;
                let subscriber: Box<Future<Item = Option<i64>, Error = telebot::Error>>;
                match args.len() {
                    1 => {
                        feed_link = args[0];
                        subscriber = futures::future::ok(Some(msg.chat.id)).boxed();
                    }
                    2 => {
                        let channel = args[0];
                        subscriber = Box::new(check_channel(&bot, channel, msg.chat.id, msg.from.unwrap().id));
                        feed_link = args[1];
                    }
                    _ => {
                        return Box::new(bot.message(msg.chat.id,
                                     "使用方法： /unsub [Channel ID] <RSS URL>".to_string())
                            .send()
                            .then(|result| match result {
                                Ok(_) => Err(None),
                                Err(e) => Err(Some(e)),
                            }))
                    }
                }
                let bot = bot.clone();
                let db = db.clone();
                let feed_link = feed_link.to_owned();
                let chat_id = msg.chat.id;
                Box::new(subscriber.then(|result| match result {
                        Ok(Some(ok)) => Ok(ok),
                        Ok(None) => Err(None),
                        Err(err) => Err(Some(err)),
                    })
                    .map(move |subscriber| (bot, db, subscriber, feed_link, chat_id)))
            })
            .and_then(move |(bot, db, subscriber, feed_link, chat_id)| {
                let r = match db.borrow_mut().unsubscribe(subscriber, &feed_link) {
                    Ok(_) => bot.message(chat_id, "退订成功".to_string()).send(),
                    Err(errors::Error(errors::ErrorKind::NotSubscribed, _)) => bot.message(chat_id, "未订阅过的 Feed".to_string()).send(),
                    Err(e) => {
                        log_error(&e);
                        bot.message(chat_id, format!("error: {}", e)).send()
                    }
                };
                r.map_err(|e| Some(e))
            })
            .then(|result| match result {
                Ok(_) => Ok(()),
                Err(None) => Ok(()),
                Err(Some(err)) => Err(err),
            });

        bot.register(handle);
    }

    loop {
        if let Err(err) = bot.run(&mut lp) {
            error!("{:?}", err);
        }
    }
}
