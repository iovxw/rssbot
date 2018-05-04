use std::time::Duration;

use futures::prelude::*;
use telebot;
use telebot::functions::*;
use tokio_core::reactor::{Handle, Interval};

use data;
use utlis::chat_is_unavailable;

pub fn spawn_subscriber_alive_checker(bot: telebot::RcBot, db: data::Database, handle: Handle) {
    let handle2 = handle.clone();
    let lop = async_block! {
        #[async]
        for _ in Interval::new(Duration::from_secs(12 * 60 * 60), &handle)
            .expect("failed to start checker loop")
        {
            let bot = bot.clone();
            let db = db.clone();
            let db2 = db.clone();
            let checker = async_block! {
                let subscribers = db.get_all_subscribers();
                for subscriber in subscribers {
                    let (_, chat) = await!(bot.get_chat(subscriber).send())
                        .map_err(move |e| (subscriber, e))?;
                    if chat.kind == "group" ||
                        chat.kind == "supergroup" ||
                        chat.kind == "channel"
                    {
                        let (_, chat_member) =
                            await!(bot.get_chat_member(subscriber, bot.inner.id).send())
                            .map_err(move |e| (subscriber, e))?;
                        if chat_member.status == "left" ||
                            chat_member.status == "kicked" ||
                            chat_member.status == "member" && chat.kind == "channel"
                        {
                            db.delete_subscriber(subscriber)
                        }
                    }
                }
                Ok(())
            }.or_else(move |(subscriber, e)| {
                if chat_is_unavailable(&e) {
                    db2.delete_subscriber(subscriber);
                } else {
                    warn!("checker {:?}", e);
                }
                Ok(())
            });
            handle.spawn(checker);
        }
        Ok(())
    }.map_err(|e: ::std::io::Error| error!("checker loop: {}", e));
    handle2.spawn(lop);
}
