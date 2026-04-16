use std::sync::Arc;

use either::Either;
use pinyin::{Pinyin, ToPinyin};
use tokio::sync::Mutex;

use crate::data::Database;
use crate::messages::{format_large_msg, Escape};

use super::{
    check_channel_permission, send_reply, update_response, CommandContext, HandlerResult,
    MsgTarget, ReplyText,
};

pub async fn rss(
    db: Arc<Mutex<Database>>,
    cmd: Arc<CommandContext>,
) -> HandlerResult {
    let chat_id = cmd.chat.id;
    let channel = cmd.text.value.trim();
    let mut target_id = chat_id;
    let target = &mut MsgTarget::new(chat_id, cmd.message_id);

    if !channel.is_empty() {
        let channel_id = check_channel_permission(&cmd, channel, target).await?;
        if channel_id.is_none() {
            return Ok(());
        }
        target_id = channel_id.unwrap();
    }

    let feeds = db.lock().await.subscribed_feeds(target_id.0);
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
    update_response(&cmd.bot, target, ReplyText::html(first_msg)).await?;

    let mut prev_msg = target.message_id;
    for msg in msgs {
        let msg = send_reply(&cmd.bot, chat_id, prev_msg, ReplyText::html(msg)).await?;
        prev_msg = msg.id;
    }
    Ok(())
}
