use std::sync::Arc;
use std::sync::Mutex;

use either::Either;
use pinyin::{Pinyin, ToPinyin};
use tbot::{
    contexts::{Command, Text},
    types::parameters,
};

use crate::data::Database;
use crate::messages::{format_large_msg, Escape};

use super::{check_channel_permission, update_response, MsgTarget};

pub async fn rss(
    db: Arc<Mutex<Database>>,
    cmd: Arc<Command<Text>>,
) -> Result<(), tbot::errors::MethodCall> {
    let chat_id = cmd.chat.id;
    let channel = &cmd.text.value;
    let mut target_id = chat_id;
    let target = &mut MsgTarget::new(chat_id, cmd.message_id);

    if !channel.is_empty() {
        let user_id = cmd.from.as_ref().unwrap().id;
        let channel_id = check_channel_permission(&cmd.bot, channel, target, user_id).await?;
        if channel_id.is_none() {
            return Ok(());
        }
        target_id = channel_id.unwrap();
    }

    let feeds = db.lock().unwrap().subscribed_feeds(target_id.0);
    let mut msgs = if let Some(mut feeds) = feeds {
        feeds.sort_by_cached_key(|feed| {
            feed.title
                .chars()
                .map(|c| {
                    c.to_pinyin()
                        .map(Pinyin::plain)
                        .map(Either::Right)
                        .unwrap_or_else(|| Either::Left(c))
                })
                .collect::<Vec<Either<char, &str>>>()
        });
        format_large_msg(tr!("subscription_list").to_string(), &feeds, |feed| {
            format!(
                "<a href=\"{}\">{}</a>",
                Escape(&feed.link),
                Escape(&feed.title)
            )
        })
    } else {
        vec![tr!("subscription_list_empty").to_string()]
    };

    let first_msg = msgs.remove(0);
    update_response(&cmd.bot, target, parameters::Text::with_html(&first_msg)).await?;

    let mut prev_msg = target.message_id;
    for msg in msgs {
        let text = parameters::Text::with_html(&msg);
        let msg = cmd
            .bot
            .send_message(chat_id, text)
            .in_reply_to(prev_msg)
            .is_web_page_preview_disabled(true)
            .call()
            .await?;
        prev_msg = msg.id;
    }
    Ok(())
}
