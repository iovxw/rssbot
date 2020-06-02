use std::sync::Arc;
use std::sync::Mutex;

use either::Either;
use pinyin::{Pinyin, ToPinyin};
use tbot::{
    contexts::{Command, Text},
    types::{
        input_file,
        parameters::{self, WebPagePreviewState},
    },
    Bot,
};

use crate::client::pull_feed;
use crate::data::Database;
use crate::messages::{format_large_msg, Escape};

mod opml;

include! { "../../ctl10n_macros.rs" }

macro_rules! reject_cmd_from_channel {
    ($cmd: tt, $target: tt) => {{
        use tbot::contexts::fields::Message;
        if $cmd.chat().kind.is_channel() {
            let msg = tr!("commands_in_private_channel");
            update_response(&$cmd.bot, $target, parameters::Text::plain(&msg)).await?;
            return Ok(());
        }
    }};
}

#[derive(Debug, Copy, Clone)]
struct MsgTarget {
    chat_id: tbot::types::chat::Id,
    message_id: tbot::types::message::Id,
    first_time: bool,
}

impl MsgTarget {
    fn new(chat_id: tbot::types::chat::Id, message_id: tbot::types::message::Id) -> Self {
        MsgTarget {
            chat_id,
            message_id,
            first_time: true,
        }
    }
    fn update(&mut self, message_id: tbot::types::message::Id) {
        self.message_id = message_id;
        self.first_time = false;
    }
}

pub async fn start(
    _db: Arc<Mutex<Database>>,
    cmd: Arc<Command<Text>>,
) -> Result<(), tbot::errors::MethodCall> {
    let target = &mut MsgTarget::new(cmd.chat.id, cmd.message_id);
    reject_cmd_from_channel!(cmd, target);
    let msg = tr!("start_message");
    update_response(&cmd.bot, target, parameters::Text::markdown(&msg)).await?;
    Ok(())
}

pub async fn rss(
    db: Arc<Mutex<Database>>,
    cmd: Arc<Command<Text>>,
) -> Result<(), tbot::errors::MethodCall> {
    let chat_id = cmd.chat.id;
    let channel = &cmd.text.value;
    let mut target_id = chat_id;
    let target = &mut MsgTarget::new(chat_id, cmd.message_id);
    reject_cmd_from_channel!(cmd, target);

    if !channel.is_empty() {
        let user_id = cmd.from.as_ref().unwrap().id;
        let channel_id = check_channel_permission(&cmd.bot, channel, target, user_id).await?;
        if channel_id.is_none() {
            return Ok(());
        }
        target_id = channel_id.unwrap();
    }

    let feeds = db.lock().unwrap().subscribed_feeds(target_id.0);
    let msgs = if let Some(mut feeds) = feeds {
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

    let mut prev_msg = cmd.message_id;
    for msg in msgs {
        let text = parameters::Text::html(&msg);
        let msg = cmd
            .bot
            .send_message(chat_id, text)
            .reply_to_message_id(prev_msg)
            .web_page_preview(WebPagePreviewState::Disabled)
            .call()
            .await?;
        prev_msg = msg.id;
    }
    Ok(())
}

pub async fn sub(
    db: Arc<Mutex<Database>>,
    cmd: Arc<Command<Text>>,
) -> Result<(), tbot::errors::MethodCall> {
    let chat_id = cmd.chat.id;
    let text = &cmd.text.value;
    let args = text.split_whitespace().collect::<Vec<_>>();
    let mut target_id = chat_id;
    let target = &mut MsgTarget::new(chat_id, cmd.message_id);
    let feed_url;
    reject_cmd_from_channel!(cmd, target);

    match &*args {
        [url] => feed_url = url,
        [channel, url] => {
            let user_id = cmd.from.as_ref().unwrap().id;
            let channel_id = check_channel_permission(&cmd.bot, channel, target, user_id).await?;
            if channel_id.is_none() {
                return Ok(());
            }
            target_id = channel_id.unwrap();
            feed_url = url;
        }
        [..] => {
            let msg = tr!("sub_how_to_use");
            update_response(&cmd.bot, target, parameters::Text::plain(&msg)).await?;
            return Ok(());
        }
    };
    if db.lock().unwrap().is_subscribed(target_id.0, feed_url) {
        update_response(
            &cmd.bot,
            target,
            parameters::Text::plain(tr!("subscribed_to_rss")),
        )
        .await?;
        return Ok(());
    }

    if cfg!(feature = "hosted-by-iovxw") && db.lock().unwrap().all_feeds().len() >= 1500 {
        let msg = tr!("subscription_rate_limit");
        update_response(&cmd.bot, target, parameters::Text::markdown(msg)).await?;
        return Ok(());
    }
    update_response(
        &cmd.bot,
        target,
        parameters::Text::plain(tr!("processing_please_wait")),
    )
    .await?;
    let msg = match pull_feed(feed_url).await {
        Ok(feed) => {
            if db.lock().unwrap().subscribe(target_id.0, feed_url, &feed) {
                tr!(
                    "subscription_succeeded",
                    link = Escape(&feed.link),
                    title = Escape(&feed.title)
                )
            } else {
                tr!("subscribed_to_rss").into()
            }
        }
        Err(e) => tr!("subscription_failed", error = Escape(&e.to_user_friendly())),
    };
    update_response(&cmd.bot, target, parameters::Text::html(&msg)).await?;
    Ok(())
}

pub async fn unsub(
    db: Arc<Mutex<Database>>,
    cmd: Arc<Command<Text>>,
) -> Result<(), tbot::errors::MethodCall> {
    let chat_id = cmd.chat.id;
    let text = &cmd.text.value;
    let args = text.split_whitespace().collect::<Vec<_>>();
    let mut target_id = chat_id;
    let target = &mut MsgTarget::new(chat_id, cmd.message_id);
    let feed_url;
    reject_cmd_from_channel!(cmd, target);

    match &*args {
        [url] => feed_url = url,
        [channel, url] => {
            let user_id = cmd.from.as_ref().unwrap().id;
            let channel_id = check_channel_permission(&cmd.bot, channel, target, user_id).await?;
            if channel_id.is_none() {
                return Ok(());
            }
            target_id = channel_id.unwrap();
            feed_url = url;
        }
        [..] => {
            let msg = tr!("unsub_how_to_use");
            update_response(&cmd.bot, target, parameters::Text::plain(&msg)).await?;
            return Ok(());
        }
    };
    let msg = if let Some(feed) = db.lock().unwrap().unsubscribe(target_id.0, feed_url) {
        tr!(
            "unsubscription_succeeded",
            link = Escape(&feed.link),
            title = Escape(&feed.title)
        )
    } else {
        tr!("unsubscribed_from_rss").into()
    };
    update_response(&cmd.bot, target, parameters::Text::html(&msg)).await?;
    Ok(())
}

pub async fn export(
    db: Arc<Mutex<Database>>,
    cmd: Arc<Command<Text>>,
) -> Result<(), tbot::errors::MethodCall> {
    let chat_id = cmd.chat.id;
    let channel = &cmd.text.value;
    let mut target_id = chat_id;
    let target = &mut MsgTarget::new(chat_id, cmd.message_id);
    reject_cmd_from_channel!(cmd, target);

    if !channel.is_empty() {
        let user_id = cmd.from.as_ref().unwrap().id;
        let channel_id = check_channel_permission(&cmd.bot, channel, target, user_id).await?;
        if channel_id.is_none() {
            return Ok(());
        }
        target_id = channel_id.unwrap();
    }

    let feeds = db.lock().unwrap().subscribed_feeds(target_id.0);
    if feeds.is_none() {
        update_response(
            &cmd.bot,
            target,
            parameters::Text::plain(tr!("subscription_list_empty")),
        )
        .await?;
        return Ok(());
    }
    let opml = opml::into_opml(feeds.unwrap());

    cmd.bot
        .send_document(
            chat_id,
            input_file::Document::bytes("feeds.opml", opml.as_bytes()),
        )
        .reply_to_message_id(cmd.message_id)
        .call()
        .await?;
    Ok(())
}

async fn update_response(
    bot: &Bot,
    target: &mut MsgTarget,
    message: parameters::Text<'_>,
) -> Result<(), tbot::errors::MethodCall> {
    let msg = if target.first_time {
        bot.send_message(target.chat_id, message)
            .reply_to_message_id(target.message_id)
            .web_page_preview(WebPagePreviewState::Disabled)
            .call()
            .await?
    } else {
        bot.edit_message_text(target.chat_id, target.message_id, message)
            .web_page_preview(WebPagePreviewState::Disabled)
            .call()
            .await?
    };
    target.update(msg.id);
    Ok(())
}

async fn check_channel_permission(
    bot: &Bot,
    channel: &str,
    target: &mut MsgTarget,
    user_id: tbot::types::user::Id,
) -> Result<Option<tbot::types::chat::Id>, tbot::errors::MethodCall> {
    use tbot::errors::MethodCall;
    let channel_id = channel
        .parse::<i64>()
        .map(|id| parameters::ChatId::Id(id.into()))
        .unwrap_or_else(|_| parameters::ChatId::Username(channel));
    update_response(
        bot,
        target,
        parameters::Text::plain(tr!("verifying_channel")),
    )
    .await?;

    let chat = match bot.get_chat(channel_id).call().await {
        Err(MethodCall::RequestError {
            description,
            error_code: 400,
            ..
        }) => {
            let msg = tr!("unable_to_find_target_channel", desc = description);
            update_response(bot, target, parameters::Text::plain(&msg)).await?;
            return Ok(None);
        }
        other => other?,
    };
    if !chat.kind.is_channel() {
        update_response(
            bot,
            target,
            parameters::Text::plain(tr!("target_must_be_a_channel")),
        )
        .await?;
        return Ok(None);
    }
    let admins = match bot.get_chat_administrators(channel_id).call().await {
        Err(MethodCall::RequestError {
            description,
            error_code: 400,
            ..
        }) => {
            let msg = tr!("unable_to_get_channel_info", desc = description);
            update_response(bot, target, parameters::Text::plain(&msg)).await?;
            return Ok(None);
        }
        other => other?,
    };
    let user_is_admin = admins
        .iter()
        .find(|member| member.user.id == user_id)
        .is_some();
    if !user_is_admin {
        update_response(
            bot,
            target,
            parameters::Text::plain(tr!("admin_only_command")),
        )
        .await?;
        return Ok(None);
    }
    let bot_is_admin = admins
        .iter()
        .find(|member| member.user.id == *crate::BOT_ID.get().unwrap())
        .is_some();
    if !bot_is_admin {
        update_response(bot, target, parameters::Text::plain(tr!("make_bot_admin"))).await?;
        return Ok(None);
    }
    Ok(Some(chat.id))
}
