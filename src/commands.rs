use std::sync::Arc;

use tbot::{
    contexts::{Command, Text},
    types::parameters,
    Bot,
};

use crate::data::Database;

mod export;
mod rss;
mod start;
mod sub;
mod unsub;

pub use export::export;
pub use rss::rss;
pub use start::start;
pub use sub::sub;
pub use unsub::unsub;

pub async fn check_command(opt: &crate::Opt, cmd: Arc<Command<Text>>) -> bool {
    use tbot::contexts::fields::Message;
    use tbot::types::chat::Kind::*;
    let target = &mut MsgTarget::new(cmd.chat.id, cmd.message_id);
    let from = cmd
        .from()
        .map(|user| user.id.0)
        .unwrap_or_else(|| cmd.chat.id.0);

    // Private mode
    if !opt.admin.is_empty() && !opt.admin.contains(&from) {
        eprintln!(
            "Unauthenticated request from user/channel: {}, command: {}, args: {}",
            from, cmd.command, cmd.text.value
        );
        return false;
    }

    match cmd.chat.kind {
        Channel { .. } => {
            let msg = tr!("commands_in_private_channel");
            let _ignore_result =
                update_response(&cmd.bot, target, parameters::Text::with_plain(&msg)).await;
            return false;
        }
        // Restrict mode: bot commands are only accessible to admins.
        Group { .. } | Supergroup { .. } if opt.restricted => {
            let user_id = cmd.from.as_ref().unwrap().id;
            let admins = match cmd.bot.get_chat_administrators(cmd.chat.id).call().await {
                Ok(r) => r,
                _ => return false,
            };
            let user_is_admin = admins.iter().any(|member| member.user.id == user_id);
            if !user_is_admin {
                let _ignore_result = update_response(
                    &cmd.bot,
                    target,
                    parameters::Text::with_plain(tr!("group_admin_only_command")),
                )
                .await;
            }
            return user_is_admin;
        }
        _ => (),
    }

    true
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

async fn update_response(
    bot: &Bot,
    target: &mut MsgTarget,
    message: parameters::Text<'_>,
) -> Result<(), tbot::errors::MethodCall> {
    let msg = if target.first_time {
        bot.send_message(target.chat_id, message)
            .in_reply_to(target.message_id)
            .is_web_page_preview_disabled(true)
            .call()
            .await?
    } else {
        bot.edit_message_text(target.chat_id, target.message_id, message)
            .is_web_page_preview_disabled(true)
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
        parameters::Text::with_plain(tr!("verifying_channel")),
    )
    .await?;

    let chat = match bot.get_chat(channel_id).call().await {
        Err(MethodCall::RequestError {
            description,
            error_code: 400,
            ..
        }) => {
            let msg = tr!("unable_to_find_target_channel", desc = description);
            update_response(bot, target, parameters::Text::with_plain(&msg)).await?;
            return Ok(None);
        }
        other => other?,
    };
    if !chat.kind.is_channel() {
        update_response(
            bot,
            target,
            parameters::Text::with_plain(tr!("target_must_be_a_channel")),
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
            update_response(bot, target, parameters::Text::with_plain(&msg)).await?;
            return Ok(None);
        }
        other => other?,
    };
    let user_is_admin = admins.iter().any(|member| member.user.id == user_id);
    if !user_is_admin {
        update_response(
            bot,
            target,
            parameters::Text::with_plain(tr!("channel_admin_only_command")),
        )
        .await?;
        return Ok(None);
    }
    let bot_is_admin = admins
        .iter()
        .find(|member| member.user.id == *crate::BOT_ID.get().unwrap())
        .is_some();
    if !bot_is_admin {
        update_response(
            bot,
            target,
            parameters::Text::with_plain(tr!("make_bot_admin")),
        )
        .await?;
        return Ok(None);
    }
    Ok(Some(chat.id))
}
