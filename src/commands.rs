use std::sync::Arc;
use std::sync::Mutex;

use tbot::{contexts::Command, types::parameters, Bot};

use crate::data::Database;

mod export;
mod rss;
mod start;
mod sub;
mod unsub;

macro_rules! add_handlers {
    ($event_loop: ident, $opt: ident, $env: ident, [$( $cmd: ident),*]) => {
        $({
            let env = $env.clone();
            let opt = $opt.clone();
            let h = move |cmd: Arc<Command>| {
                let env = env.clone();
                let opt = opt.clone();
                async move {
                    if check_command(&opt, &cmd).await {
                        if let Err(e) = self::$cmd::$cmd(env, cmd).await {
                            crate::print_error(e);
                        }
                    }
                }
            };
            $event_loop.command(stringify!($cmd), h);
        })*
    };
}

pub fn register_commands(
    event_loop: &mut tbot::EventLoop,
    opt: Arc<crate::Opt>,
    db: Arc<Mutex<Database>>,
) {
    add_handlers!(event_loop, opt, db, [start, rss, sub, unsub, export]);
}

pub async fn check_command(opt: &crate::Opt, cmd: &Command) -> bool {
    use tbot::types::chat::Kind::*;
    let reply_target = &mut MsgTarget::new(cmd.chat.id, cmd.message_id);

    // Private mode
    if !opt.admin.is_empty() && !is_from_bot_admin(&cmd, &opt.admin) {
        eprintln!(
            "Unauthenticated request from user/channel: {:?}, command: {}, args: {}",
            cmd.from, cmd.command, cmd.text.value
        );
        return false;
    }

    match cmd.chat.kind {
        Channel { .. } => {
            let msg = tr!("commands_in_private_channel");
            let _ignore_result =
                update_response(&cmd.bot, reply_target, parameters::Text::with_plain(msg)).await;
            return false;
        }
        // Restrict mode: bot commands are only accessible to admins.
        Group { .. } | Supergroup { .. } if opt.restricted => {
            let user_is_admin = is_from_chat_admin(&cmd).await;
            if !user_is_admin {
                let _ignore_result = update_response(
                    &cmd.bot,
                    reply_target,
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

fn is_from_bot_admin(cmd: &Command, admins: &[i64]) -> bool {
    use tbot::types::message::From;
    match &cmd.from {
        Some(from) => {
            let id = match from {
                From::User(user) => user.id.0,
                From::Chat(chat) => chat.id.0,
            };
            admins.contains(&id)
        }
        None => false,
    }
}

async fn is_from_chat_admin(cmd: &Command) -> bool {
    use tbot::types::message::From;
    match &cmd.from {
        Some(From::User(user)) => {
            let admins = match cmd.bot.get_chat_administrators(cmd.chat.id).call().await {
                Ok(r) => r,
                _ => return false,
            };
            admins.iter().any(|member| member.user.id == user.id)
        }
        Some(From::Chat(chat)) => chat.id == cmd.chat.id,
        None => false,
    }
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
    message: parameters::Text,
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
    cmd: &Command,
    channel: &str,
    target: &mut MsgTarget,
) -> Result<Option<tbot::types::chat::Id>, tbot::errors::MethodCall> {
    use tbot::errors::MethodCall;
    let bot = &cmd.bot;
    let from = cmd
        .from
        .as_ref()
        .expect("UNREACHABLE: message from channel");

    if from.is_chat() {
        // FIXME: error message
        return Ok(None);
    }

    let user_id = from.clone().expect_user().id;

    let channel_id = channel
        .parse::<i64>()
        .map(|id| parameters::ChatId::Id(id.into()))
        .unwrap_or_else(|_| parameters::ChatId::Username(channel.to_string()));

    update_response(
        bot,
        target,
        parameters::Text::with_plain(tr!("verifying_channel")),
    )
    .await?;

    let chat = match bot.get_chat(channel_id.clone()).call().await {
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
