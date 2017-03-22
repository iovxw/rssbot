use telebot::functions::*;
use tokio_core::reactor::Handle;
use futures::{self, Future, Stream, IntoFuture};
use tokio_curl::Session;
use telebot;
use pinyin_order;

use errors;
use feed;
use utlis::{Escape, EscapeUrl, send_multiple_messages, format_and_split_msgs, to_chinese_error_msg,
            log_error};
use data::Database;

pub fn register_commands(bot: telebot::RcBot, db: Database, lphandle: Handle) {
    register_rss(bot.clone(), db.clone());
    register_sub(bot.clone(), db.clone(), lphandle);
    register_unsub(bot.clone(), db.clone());
    register_unsubthis(bot.clone(), db.clone());
}

fn register_rss(bot: telebot::RcBot, db: Database) {
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
                        subscriber = Box::new(check_channel(bot.clone(),
                                                            channel,
                                                            msg.chat.id,
                                                            msg.from.unwrap().id));
                    }
                }
                2 => {
                    raw = true;
                    let channel = args[0];
                    subscriber = Box::new(check_channel(bot.clone(),
                                                        channel,
                                                        msg.chat.id,
                                                        msg.from.unwrap().id));
                }
                _ => {
                    return Box::new(bot.message(msg.chat.id,
                                                "使用方法: /rss <Channel ID> <raw>"
                                                    .to_string())
                                        .send()
                                        .then(|result| match result {
                                                  Ok(_) => Err(None),
                                                  Err(e) => Err(Some(e)),
                                              }))
                }
            }
            let db = db.clone();
            let chat_id = msg.chat.id;
            Box::new(subscriber.then(|result| match result {
                                         Ok(Some(ok)) => Ok(ok),
                                         Ok(None) => Err(None),
                                         Err(err) => Err(Some(err)),
                                     })
                         .map(move |subscriber| (bot, db, subscriber, raw, chat_id)))
        })
        .and_then(|(bot, db, subscriber, raw, chat_id)| {
            db.get_subscribed_feeds(subscriber)
                .map({
                         let bot = bot.clone();
                         move |feeds| Ok((bot, raw, chat_id, feeds))
                     })
                .unwrap_or(Err((bot, chat_id)))
                .into_future()
                .or_else(|(bot, chat_id)| {
                    bot.message(chat_id, "订阅列表为空".to_string())
                                .send()
                                .then(|r| match r {
                                          Ok(_) => Err(None),
                                          Err(e) => Err(Some(e)),
                                      })
                })
        })
        .and_then(|(bot, raw, chat_id, mut feeds)| {
            let text = String::from("订阅列表:");
            if !raw {
                    feeds.sort_by_key(|feed| pinyin_order::as_pinyin(&feed.title));
                    let msgs = format_and_split_msgs(text, &feeds, |feed| {
                        format!("<a href=\"{}\">{}</a>",
                                EscapeUrl(&feed.link),
                                Escape(&feed.title))
                    });
                    send_multiple_messages(&bot, chat_id, msgs)
                } else {
                    feeds.sort_by(|a, b| a.link.cmp(&b.link));
                    let msgs = format_and_split_msgs(text, &feeds, |feed| {
                        format!("{}: {}", Escape(&feed.title), Escape(&feed.link))
                    });
                    send_multiple_messages(&bot, chat_id, msgs)
                }
                .map_err(|e| Some(e))
        })
        .then(|result| match result {
                  Err(Some(err)) => {
            error!("telebot: {:?}", err);
            Ok::<(), ()>(())
        }
                  _ => Ok(()),
              });

    bot.register(handle);
}

fn register_sub(bot: telebot::RcBot, db: Database, lphandle: Handle) {
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
                    subscriber = Box::new(check_channel(bot.clone(),
                                                        channel,
                                                        msg.chat.id,
                                                        msg.from.unwrap().id));
                    feed_link = args[1];
                }
                _ => {
                    return Box::new(bot.message(msg.chat.id,
                                                "使用方法: /sub [Channel ID] <RSS URL>"
                                                    .to_string())
                                        .send()
                                        .then(|result| match result {
                                                  Ok(_) => Err(None),
                                                  Err(e) => Err(Some(e)),
                                              }))
                }
            }
            let db = db.clone();
            let feed_link = feed_link.to_owned();
            let chat_id = msg.chat.id;
            let lphandle = lphandle.clone();
            Box::new(subscriber.then(|result| match result {
                                         Ok(Some(ok)) => Ok(ok),
                                         Ok(None) => Err(None),
                                         Err(err) => Err(Some(err)),
                                     })
                         .map(move |subscriber| {
                                  (bot, db, subscriber, feed_link, chat_id, lphandle)
                              }))
        })
        .and_then(|(bot, db, subscriber, feed_link, chat_id, lphandle)| {
            if db.is_subscribed(subscriber, &feed_link) {
                    Err((bot, chat_id))
                } else {
                    Ok((bot, db, subscriber, feed_link, chat_id, lphandle))
                }
                .into_future()
                .or_else(|(bot, chat_id)| {
                    bot.message(chat_id, "已订阅过的 RSS".to_string())
                            .send()
                            .then(|result| match result {
                                      Ok(_) => Err(None),
                                      Err(e) => Err(Some(e)),
                                  })
                })
        })
        .and_then(|(bot, db, subscriber, feed_link, chat_id, lphandle)| {
            let session = Session::new(lphandle);
            let bot2 = bot.clone();
            feed::fetch_feed(session, feed_link.to_owned())
                .map(move |feed| (bot2, db, subscriber, feed_link, chat_id, feed))
                .or_else(move |e| {
                    bot.message(chat_id,
                                 format!("订阅失败: {}", to_chinese_error_msg(e)))
                        .send()
                        .then(|result| match result {
                                  Ok(_) => Err(None),
                                  Err(e) => Err(Some(e)),
                              })
                })
        })
        .and_then(|(bot, db, subscriber, feed_link, chat_id, feed)| {
            match db.subscribe(subscriber, &feed_link, &feed) {
                    Ok(_) => {
                        bot.message(chat_id,
                                     format!("《<a href=\"{}\">{}</a>》订阅成功",
                                             EscapeUrl(&feed.link),
                                             Escape(&feed.title)))
                            .parse_mode("HTML")
                            .disable_web_page_preview(true)
                            .send()
                    }
                    Err(e) => {
                        log_error(&e);
                        bot.message(chat_id, format!("error: {}", e)).send()
                    }
                }
                .map_err(|e| Some(e))
        })
        .then(|result| match result {
                  Err(Some(err)) => {
            error!("telebot: {:?}", err);
            Ok::<(), ()>(())
        }
                  _ => Ok(()),
              });

    bot.register(handle);
}

fn register_unsub(bot: telebot::RcBot, db: Database) {
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
                    subscriber = Box::new(check_channel(bot.clone(),
                                                        channel,
                                                        msg.chat.id,
                                                        msg.from.unwrap().id));
                    feed_link = args[1];
                }
                _ => {
                    return Box::new(bot.message(msg.chat.id,
                                                "使用方法: /unsub [Channel ID] <RSS URL>"
                                                    .to_string())
                                        .send()
                                        .then(|result| match result {
                                                  Ok(_) => Err(None),
                                                  Err(e) => Err(Some(e)),
                                              }))
                }
            }
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
        .and_then(|(bot, db, subscriber, feed_link, chat_id)| {
            match db.unsubscribe(subscriber, &feed_link) {
                    Ok(feed) => {
                        bot.message(chat_id,
                                     format!("《<a href=\"{}\">{}</a>》退订成功",
                                             EscapeUrl(&feed.link),
                                             Escape(&feed.title)))
                            .parse_mode("HTML")
                            .disable_web_page_preview(true)
                            .send()
                    }
                    Err(errors::Error(errors::ErrorKind::NotSubscribed, _)) => {
                        bot.message(chat_id, "未订阅过的 RSS".to_string()).send()
                    }
                    Err(e) => {
                        log_error(&e);
                        bot.message(chat_id, format!("error: {}", e)).send()
                    }
                }
                .map_err(|e| Some(e))
        })
        .then(|result| match result {
                  Err(Some(err)) => {
            error!("telebot: {:?}", err);
            Ok::<(), ()>(())
        }
                  _ => Ok(()),
              });

    bot.register(handle);
}

fn register_unsubthis(bot: telebot::RcBot, db: Database) {
    let handle = bot.new_cmd("/unsubthis")
        .map_err(|e| Some(e))
        .and_then(move |(bot, msg)| {
            if let Some(reply_msg) = msg.reply_to_message {
                    Ok((bot, db.clone(), msg.chat.id, reply_msg))
                } else {
                    Err((bot, msg.chat.id))
                }
                .into_future()
                .or_else(|(bot, chat_id)| {
                    bot.message(chat_id,
                                 "使用方法: \
                                      使用此命令回复想要退订的 RSS 消息即可退订,\
                                      不支持 Channel"
                                         .to_string())
                        .send()
                        .then(|result| match result {
                                  Ok(_) => Err(None),
                                  Err(e) => Err(Some(e)),
                              })
                })

        })
        .and_then(|(bot, db, chat_id, reply_msg)| {
            if let Some(m) = reply_msg.text {
                    if let Some(title) = m.lines().next() {
                        Ok((bot, db, chat_id, title.to_string()))
                    } else {
                        Err((bot, chat_id))
                    }
                } else {
                    Err((bot, chat_id))
                }
                .into_future()
                .or_else(|(bot, chat_id)| {
                    bot.message(chat_id, "无法识别的消息".to_string())
                        .send()
                        .then(|result| match result {
                                  Ok(_) => Err(None),
                                  Err(e) => Err(Some(e)),
                              })
                })
        })
        .and_then(|(bot, db, chat_id, title)| {
            if let Some(feed_link) = db.get_subscribed_feeds(chat_id)
                       .unwrap_or_default()
                       .iter()
                       .filter(|feed| feed.title == title)
                       .map(|feed| feed.link.clone())
                       .next() {
                    Ok((bot, db, chat_id, feed_link))
                } else {
                    Err((bot, chat_id))
                }
                .into_future()
                .or_else(|(bot, chat_id)| {
                    bot.message(chat_id, "无法找到此订阅".to_string())
                        .send()
                        .then(|result| match result {
                                  Ok(_) => Err(None),
                                  Err(e) => Err(Some(e)),
                              })
                })
        })
        .and_then(|(bot, db, chat_id, feed_link)| {
            match db.unsubscribe(chat_id, &feed_link) {
                    Ok(feed) => {
                        bot.message(chat_id,
                                     format!("《<a href=\"{}\">{}</a>》退订成功",
                                             EscapeUrl(&feed.link),
                                             Escape(&feed.title)))
                            .parse_mode("HTML")
                            .disable_web_page_preview(true)
                            .send()
                    }
                    Err(e) => {
                        log_error(&e);
                        bot.message(chat_id, format!("error: {}", e)).send()
                    }
                }
                .map_err(|e| Some(e))
        })
        .then(|result| match result {
                  Err(Some(err)) => {
            error!("telebot: {:?}", err);
            Ok::<(), ()>(())
        }
                  _ => Ok(()),
              });

    bot.register(handle);
}

fn check_channel<'a>(bot: telebot::RcBot,
                     channel: &str,
                     chat_id: i64,
                     user_id: i64)
                     -> impl Future<Item = Option<i64>, Error = telebot::Error> + 'a {
    let channel = channel.parse::<i64>()
        .map(|_| if !channel.starts_with("-100") {
                 format!("-100{}", channel)
             } else {
                 channel.to_owned()
             })
        .unwrap_or_else(|_| if !channel.starts_with("@") {
                            format!("@{}", channel)
                        } else {
                            channel.to_owned()
                        });
    bot.message(chat_id, "正在验证 Channel".to_string())
        .send()
        .map_err(|e| Some(e))
        .and_then(move |(bot, msg)| {
            let msg_id = msg.message_id;
            bot.get_chat_string(channel)
                .send()
                .or_else(move |e| {
                    if let telebot::Error::Telegram(err_msg) = e {
                            Err((bot, chat_id, msg_id, err_msg))
                        } else {
                            Ok(e)
                        }
                        .into_future()
                        .or_else(|(bot, chat_id, msg_id, err_msg)| {
                            bot.edit_message_text(chat_id,
                                                   msg_id,
                                                   format!("无法找到目标 Channel: {}",
                                                           err_msg))
                                .send()
                                .then(|result| match result {
                                          Ok(_) => Err(None),
                                          Err(e) => Err(Some(e)),
                                      })
                        })
                        .and_then(|e| Err(Some(e)))
                })
                .map(move |(bot, channel)| (bot, chat_id, user_id, channel, msg_id))
        })
        .and_then(|(bot, chat_id, user_id, channel, msg_id)| {
            if channel.kind == "channel" {
                    Ok((bot, chat_id, user_id, channel.id, msg_id))
                } else {
                    Err((bot, chat_id, msg_id))
                }
                .into_future()
                .or_else(|(bot, chat_id, msg_id)| {
                    bot.edit_message_text(chat_id, msg_id, "目标需为 Channel".to_string())
                        .send()
                        .then(|result| match result {
                                  Ok(_) => Err(None),
                                  Err(e) => Err(Some(e)),
                              })
                })
        })
        .and_then(|(bot, chat_id, user_id, channel_id, msg_id)| {
            bot.unban_chat_administrators(channel_id)
                .send()
                .or_else(move |e| {
                    if let telebot::Error::Telegram(err_msg) = e {
                            Err((bot, chat_id, msg_id, err_msg))
                        } else {
                            Ok(e)
                        }
                        .into_future()
                        .or_else(|(bot, chat_id, msg_id, err_msg)| {
                            bot.edit_message_text(chat_id,
                                                   msg_id,
                                                   format!("请先将本 Bot 加入目标 Channel\
                                                       并设为管理员: {}",
                                                           err_msg))
                                .send()
                                .then(|result| match result {
                                          Ok(_) => Err(None),
                                          Err(e) => Err(Some(e)),
                                      })
                        })
                        .and_then(|e| Err(Some(e)))
                })
                .map(move |(bot, admins)| {
                         let admin_id_list =
                        admins.iter().map(|member| member.user.id).collect::<Vec<i64>>();
                         (bot, chat_id, user_id, admin_id_list, msg_id, channel_id)
                     })
        })
        .and_then(|(bot, chat_id, user_id, admin_id_list, msg_id, channel_id)| {
            if admin_id_list.contains(&bot.inner.id) {
                    Ok((bot, chat_id, user_id, admin_id_list, msg_id, channel_id))
                } else {
                    Err((bot, chat_id, msg_id))
                }
                .into_future()
                .or_else(|(bot, chat_id, msg_id)| {
                    bot.edit_message_text(chat_id,
                                           msg_id,
                                           "请将本 Bot 设为管理员".to_string())
                        .send()
                        .then(|result| match result {
                                  Ok(_) => Err(None),
                                  Err(e) => Err(Some(e)),
                              })
                })
        })
        .and_then(|(bot, chat_id, user_id, admin_id_list, msg_id, channel_id)| {
            if admin_id_list.contains(&user_id) {
                    Ok(channel_id)
                } else {
                    Err((bot, chat_id, msg_id))
                }
                .into_future()
                .or_else(|(bot, chat_id, msg_id)| {
                    bot.edit_message_text(chat_id,
                                           msg_id,
                                           "该命令只能由 Channel 管理员使用".to_string())
                        .send()
                        .then(|result| match result {
                                  Ok(_) => Err(None),
                                  Err(e) => Err(Some(e)),
                              })
                })
        })
        .then(|result| match result {
                  Err(None) => Ok(None),
                  Err(Some(e)) => Err(e),
                  Ok(ok) => Ok(Some(ok)),
              })
}
