use std::sync::Arc;

use teloxide::{
    prelude::*,
    sugar::request::{RequestLinkPreviewExt as _, RequestReplyExt as _},
    types::{Chat, MessageId, ParseMode, Recipient, User},
    RequestError,
};
use tokio::sync::Mutex;

use crate::data::Database;

mod export;
mod rss;
mod start;
mod sub;
mod unsub;

pub async fn handle_message(
    bot: Bot,
    msg: Message,
    opt: Arc<crate::Opt>,
    db: Arc<Mutex<Database>>,
) {
    if let Err(err) = dispatch_command(bot, msg, opt, db).await {
        crate::print_error(err);
    }
}

async fn dispatch_command(
    bot: Bot,
    msg: Message,
    opt: Arc<crate::Opt>,
    db: Arc<Mutex<Database>>,
) -> HandlerResult {
    let Some(text) = msg.text().map(str::to_owned) else {
        return Ok(());
    };

    let bot_username = crate::BOT_NAME.get().map(String::as_str);
    let Some((command, args)) = ["start", "rss", "sub", "unsub", "export"]
        .into_iter()
        .find_map(|command| {
            crate::command_text::parse_matching_command(&text, command, bot_username)
                .map(|args| (command, args))
        })
    else {
        return Ok(());
    };

    let cmd = Arc::new(Command::from_message(bot, msg, command, args));
    if !check_command(&opt, &cmd).await {
        return Ok(());
    }

    match command {
        "start" => start::start(db, cmd).await,
        "rss" => rss::rss(db, cmd).await,
        "sub" => sub::sub(db, cmd).await,
        "unsub" => unsub::unsub(db, cmd).await,
        "export" => export::export(db, cmd).await,
        _ => Ok(()),
    }
}

pub(super) type HandlerResult = Result<(), RequestError>;

#[derive(Debug, Clone)]
pub(super) struct Command {
    pub(super) bot: Bot,
    pub(super) chat: Chat,
    pub(super) from: Option<MessageFrom>,
    pub(super) message_id: MessageId,
    pub(super) text: CommandText,
    pub(super) command: String,
}

impl Command {
    fn from_message(bot: Bot, msg: Message, command: &str, args: &str) -> Self {
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
            text: CommandText {
                value: args.to_owned(),
            },
            command: command.to_owned(),
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct CommandText {
    pub(super) value: String,
}

#[derive(Debug, Clone)]
pub(super) enum MessageFrom {
    User(User),
    Chat(Chat),
}

impl MessageFrom {
    fn is_chat(&self) -> bool {
        matches!(self, Self::Chat(_))
    }

    fn expect_user(self) -> User {
        match self {
            Self::User(user) => user,
            Self::Chat(_) => panic!("UNREACHABLE: expected user sender"),
        }
    }
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

pub async fn check_command(opt: &crate::Opt, cmd: &Command) -> bool {
    let reply_target = &mut MsgTarget::new(cmd.chat.id, cmd.message_id);

    // Private mode
    if !opt.admin.is_empty() && !is_from_bot_admin(cmd, &opt.admin) {
        eprintln!(
            "Unauthenticated request from user/channel: {:?}, command: {}, args: {}",
            cmd.from, cmd.command, cmd.text.value
        );
        return false;
    }

    if cmd.chat.is_channel() {
        let msg = tr!("commands_in_private_channel");
        let _ignore_result = update_response(&cmd.bot, reply_target, ReplyText::plain(msg)).await;
        return false;
    }

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

fn is_from_bot_admin(cmd: &Command, admins: &[i64]) -> bool {
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

async fn is_from_chat_admin(cmd: &Command) -> bool {
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
    cmd: &Command,
    channel: &str,
    target: &mut MsgTarget,
) -> Result<Option<ChatId>, RequestError> {
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
