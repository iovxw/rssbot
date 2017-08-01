use std::time::Duration;

use telebot;
use telebot::functions::*;
use tokio_core::reactor::{Interval, Handle};
use futures::{self, Future, Stream, IntoFuture};

use data;
use utlis::chat_is_unavailable;

pub fn spawn_subscriber_alive_checker(bot: telebot::RcBot, db: data::Database, handle: Handle) {
    handle.clone().spawn(
        Interval::new(Duration::from_secs(12 * 60 * 60), &handle)
            .expect("failed to start checker loop")
            .map_err(|e| error!("checker loop error: {}", e))
            .for_each(move |_| {
                let subscribers = db.get_all_subscribers();
                let bot = bot.clone();
                let db = db.clone();
                let checker = futures::stream::iter(subscribers.into_iter().map(Ok))
                    .for_each(move |subscriber| {
                        let db = db.clone();
                        let db2 = db.clone();
                        bot.get_chat(subscriber)
                            .send()
                            .map_err(move |e| {
                                match e {
                                    telebot::error::Error::Telegram(_, ref s, _)
                                        if chat_is_unavailable(s) => {
                                        db.delete_subscriber(subscriber);
                                    }
                                    _ => (),
                                };
                            })
                            .and_then(move |(bot, chat)| {
                                match chat.kind.as_str() {
                                    "group" | "supergroup" | "channel" => Err(()),
                                    "private" | _ => Ok(()),
                                }.into_future()
                                    .or_else(move |_| {
                                        bot.get_chat_member(subscriber, bot.inner.id)
                                            .send()
                                            .map(move |(_, chat_member)| match chat_member
                                                .status
                                                .as_str() {
                                                "left" | "kicked" => {
                                                    db2.delete_subscriber(subscriber)
                                                }
                                                "member" if chat.kind == "channel" => {
                                                    db2.delete_subscriber(subscriber)
                                                }
                                                "creator" | "administrator" | _ => (),
                                            })
                                            .map_err(|_| ())
                                    })
                            })
                    });
                handle.spawn(checker);
                Ok(())
            }),
    )
}
