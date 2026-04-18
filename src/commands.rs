use std::sync::Arc;

use teloxide::{
    prelude::*,
    sugar::request::{RequestLinkPreviewExt as _, RequestReplyExt as _},
    types::{Chat, MessageId, ParseMode, Recipient, User},
    utils::command::BotCommands,
    RequestError,
};
use tokio::sync::Mutex;

use crate::data::Database;

mod export;
mod rss;
mod start;
mod sub;
mod unsub;

#[derive(BotCommands, Clone, Debug, PartialEq, Eq)]
#[command(rename_rule = "lowercase")]
pub(crate) enum BotCommand {
    Start,
    Rss(String),
    Sub(String),
    Unsub(String),
    Export(String),
}

pub async fn handle_message(
    bot: Bot,
    msg: Message,
    cmd: BotCommand,
    opt: Arc<crate::Opt>,
    db: Arc<Mutex<Database>>,
) {
    if let Err(err) = dispatch_command(bot, msg, cmd, opt, db).await {
        crate::print_error(err);
    }
}

async fn dispatch_command(
    bot: Bot,
    msg: Message,
    cmd: BotCommand,
    opt: Arc<crate::Opt>,
    db: Arc<Mutex<Database>>,
) -> HandlerResult {
    let cmd = Arc::new(CommandContext::from_message(bot, msg, cmd));
    if !check_command(&opt, &cmd).await {
        return Ok(());
    }

    match cmd.command.clone() {
        BotCommand::Start => start::start(db, cmd).await,
        BotCommand::Rss(_) => rss::rss(db, cmd).await,
        BotCommand::Sub(_) => sub::sub(db, cmd).await,
        BotCommand::Unsub(_) => unsub::unsub(db, cmd).await,
        BotCommand::Export(_) => export::export(db, cmd).await,
    }
}

pub(super) type HandlerResult = Result<(), RequestError>;

#[derive(Debug, Clone)]
pub(super) struct CommandContext {
    pub(super) bot: Bot,
    pub(super) chat: Chat,
    pub(super) from: Option<MessageFrom>,
    pub(super) message_id: MessageId,
    pub(super) command: BotCommand,
}

impl CommandContext {
    fn from_message(bot: Bot, msg: Message, command: BotCommand) -> Self {
        // Preserve the old tbot behavior where channel-signed messages are
        // treated as coming from sender_chat instead of an optional user.
        let from = msg
            .sender_chat
            .clone()
            .map(MessageFrom::Chat)
            .or_else(|| msg.from.clone().map(MessageFrom::User));

        Self {
            bot,
            chat: msg.chat.clone(),
            from,
            message_id: msg.id,
            command,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) enum MessageFrom {
    User(User),
    Chat(Chat),
}

pub(super) enum ReplyText {
    Plain(String),
    Html(String),
    Markdown(String),
}

impl ReplyText {
    pub(super) fn plain(text: impl Into<String>) -> Self {
        Self::Plain(text.into())
    }

    pub(super) fn html(text: impl Into<String>) -> Self {
        Self::Html(text.into())
    }

    pub(super) fn markdown(text: impl Into<String>) -> Self {
        Self::Markdown(text.into())
    }
}

pub async fn check_command(opt: &crate::Opt, cmd: &CommandContext) -> bool {
    let reply_target = &mut MsgTarget::new(cmd.chat.id, cmd.message_id);

    // Private mode
    if !opt.admin.is_empty() && !is_from_bot_admin(cmd, &opt.admin) {
        eprintln!(
            "Unauthenticated request from user/channel: {:?}, command: {:?}",
            cmd.from, cmd.command
        );
        return false;
    }

    if cmd.chat.is_channel() {
        let msg = tr!("commands_in_private_channel");
        let _ignore_result = update_response(&cmd.bot, reply_target, ReplyText::plain(msg)).await;
        return false;
    }

    // Restrict mode: bot commands are only accessible to admins.
    if opt.restricted && (cmd.chat.is_group() || cmd.chat.is_supergroup()) {
        let user_is_admin = is_from_chat_admin(cmd).await;
        if !user_is_admin {
            let _ignore_result = update_response(
                &cmd.bot,
                reply_target,
                ReplyText::plain(tr!("group_admin_only_command")),
            )
            .await;
        }
        return user_is_admin;
    }

    true
}

fn is_from_bot_admin(cmd: &CommandContext, admins: &[i64]) -> bool {
    match &cmd.from {
        Some(from) => {
            let id = match from {
                MessageFrom::User(user) => user.id.0 as i64,
                MessageFrom::Chat(chat) => chat.id.0,
            };
            admins.contains(&id)
        }
        None => false,
    }
}

async fn is_from_chat_admin(cmd: &CommandContext) -> bool {
    match &cmd.from {
        Some(MessageFrom::User(user)) => {
            let user_id = user.id;
            let admins = match cmd.bot.get_chat_administrators(cmd.chat.id).await {
                Ok(r) => r,
                _ => return false,
            };
            admins.iter().any(|member| member.user.id == user_id)
        }
        Some(MessageFrom::Chat(chat)) => chat.id == cmd.chat.id,
        None => false,
    }
}

#[derive(Debug, Copy, Clone)]
pub(super) struct MsgTarget {
    pub(super) chat_id: ChatId,
    pub(super) message_id: MessageId,
    first_time: bool,
}

impl MsgTarget {
    pub(super) fn new(chat_id: ChatId, message_id: MessageId) -> Self {
        MsgTarget {
            chat_id,
            message_id,
            first_time: true,
        }
    }
    fn update(&mut self, message_id: MessageId) {
        self.message_id = message_id;
        self.first_time = false;
    }
}

pub(super) async fn update_response(
    bot: &Bot,
    target: &mut MsgTarget,
    message: ReplyText,
) -> HandlerResult {
    // Keep a single status message per command: reply once, then edit it as the
    // command progresses.
    let msg = if target.first_time {
        send_reply(bot, target.chat_id, target.message_id, message).await?
    } else {
        edit_reply(bot, target.chat_id, target.message_id, message).await?
    };
    target.update(msg.id);
    Ok(())
}

#[allow(deprecated)]
pub(super) async fn send_reply(
    bot: &Bot,
    chat_id: ChatId,
    reply_to: MessageId,
    message: ReplyText,
) -> Result<Message, RequestError> {
    match message {
        ReplyText::Plain(text) => bot
            .send_message(chat_id, text)
            .reply_to(reply_to)
            .disable_link_preview(true)
            .await,
        ReplyText::Html(text) => bot
            .send_message(chat_id, text)
            .reply_to(reply_to)
            .parse_mode(ParseMode::Html)
            .disable_link_preview(true)
            .await,
        ReplyText::Markdown(text) => bot
            .send_message(chat_id, text)
            .reply_to(reply_to)
            .parse_mode(ParseMode::Markdown)
            .disable_link_preview(true)
            .await,
    }
}

#[allow(deprecated)]
async fn edit_reply(
    bot: &Bot,
    chat_id: ChatId,
    message_id: MessageId,
    message: ReplyText,
) -> Result<Message, RequestError> {
    match message {
        ReplyText::Plain(text) => bot
            .edit_message_text(chat_id, message_id, text)
            .disable_link_preview(true)
            .await,
        ReplyText::Html(text) => bot
            .edit_message_text(chat_id, message_id, text)
            .parse_mode(ParseMode::Html)
            .disable_link_preview(true)
            .await,
        ReplyText::Markdown(text) => bot
            .edit_message_text(chat_id, message_id, text)
            .parse_mode(ParseMode::Markdown)
            .disable_link_preview(true)
            .await,
    }
}

fn request_error_description(err: &RequestError) -> String {
    match err {
        RequestError::Api(api) => api.to_string(),
        _ => err.to_string(),
    }
}

pub(super) async fn check_channel_permission(
    cmd: &CommandContext,
    channel: &str,
    target: &mut MsgTarget,
) -> Result<Option<ChatId>, RequestError> {
    let bot = &cmd.bot;
    let from = cmd
        .from
        .as_ref()
        .expect("UNREACHABLE: message from channel");
    let user_id = match from.clone() {
        MessageFrom::User(user) => user.id,
        MessageFrom::Chat(_) => {
            // FIXME: error message
            return Ok(None);
        }
    };

    let channel_id = channel
        .parse::<i64>()
        .map(|id| Recipient::Id(ChatId(id)))
        .unwrap_or_else(|_| Recipient::ChannelUsername(channel.to_string()));

    update_response(
        bot,
        target,
        ReplyText::plain(tr!("verifying_channel")),
    )
    .await?;

    let chat = match bot.get_chat(channel_id.clone()).await {
        Err(err @ RequestError::Api(_)) => {
            let msg = tr!(
                "unable_to_find_target_channel",
                desc = request_error_description(&err)
            );
            update_response(bot, target, ReplyText::plain(msg)).await?;
            return Ok(None);
        }
        other => other?,
    };
    if !chat.is_channel() {
        update_response(
            bot,
            target,
            ReplyText::plain(tr!("target_must_be_a_channel")),
        )
        .await?;
        return Ok(None);
    }
    let admins = match bot.get_chat_administrators(channel_id).await {
        Err(err @ RequestError::Api(_)) => {
            let msg = tr!(
                "unable_to_get_channel_info",
                desc = request_error_description(&err)
            );
            update_response(bot, target, ReplyText::plain(msg)).await?;
            return Ok(None);
        }
        other => other?,
    };
    let user_is_admin = admins.iter().any(|member| member.user.id == user_id);
    if !user_is_admin {
        update_response(
            bot,
            target,
            ReplyText::plain(tr!("channel_admin_only_command")),
        )
        .await?;
        return Ok(None);
    }
    let bot_is_admin = admins
        .iter()
        .any(|member| member.user.id == crate::BOT_ID.get().cloned().unwrap());
    if !bot_is_admin {
        update_response(
            bot,
            target,
            ReplyText::plain(tr!("make_bot_admin")),
        )
        .await?;
        return Ok(None);
    }
    Ok(Some(chat.id))
}

#[cfg(test)]
mod tests {
    use super::BotCommand;
    use teloxide::utils::command::BotCommands as _;

    #[test]
    fn native_parser_accepts_case_insensitive_mentions() {
        assert_eq!(
            BotCommand::parse("/sub@RSSBOT https://example.com/rss.xml", "rssbot").unwrap(),
            BotCommand::Sub("https://example.com/rss.xml".to_owned())
        );
    }

    #[test]
    fn native_parser_preserves_leading_argument_whitespace() {
        assert_eq!(
            BotCommand::parse("/rss   @example_channel", "rssbot").unwrap(),
            BotCommand::Rss("  @example_channel".to_owned())
        );
    }
}
