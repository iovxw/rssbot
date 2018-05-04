//! Available telegram functions, copied from https://core.telegram.org/bots/api#available-methods
//!
//! telebot-derive implements setter, setter and send methods to each struct

use bot::{Bot, RcBot};
use error::Error;
use futures::Future;
use objects;
use objects::{Boolean, Integer, NotImplemented};
use serde;
use serde_json;
use std::rc::Rc;

pub trait TelegramSendable {
    type Item;

    fn send(self) -> Box<Future<Item = Self::Item, Error = Error>>;
}

pub enum ChatID {
    String(String),
    Integer(i64),
}

impl serde::Serialize for ChatID {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match *self {
            ChatID::String(ref id) => serializer.serialize_str(id),
            ChatID::Integer(id) => serializer.serialize_i64(id),
        }
    }
}

impl From<String> for ChatID {
    fn from(id: String) -> Self {
        ChatID::String(id)
    }
}

impl From<i64> for ChatID {
    fn from(id: i64) -> Self {
        ChatID::Integer(id)
    }
}

pub enum File {
    String(String),
    InputFile(String, Vec<u8>),
}

impl From<String> for File {
    fn from(id: String) -> Self {
        File::String(id)
    }
}

impl serde::Serialize for File {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match *self {
            File::String(ref id) => serializer.serialize_str(id),
            File::InputFile(..) => unreachable!(),
        }
    }
}

impl File {
    pub fn new(name: String, data: Vec<u8>) -> File {
        File::InputFile(name, data)
    }
}

/// The strongly typed version of the parse_mode field which indicates the type of text
pub enum ParseMode {
    Markdown,
    HTML,
    Text,
}

impl Into<String> for ParseMode {
    fn into(self) -> String {
        let tmp = match self {
            ParseMode::Markdown => "Markdown",
            ParseMode::HTML => "HTML",
            ParseMode::Text => "Text",
        };

        tmp.into()
    }
}

/// The strongly typed version of the action field which indicates the type of action
pub enum Action {
    Typing,
    UploadPhoto,
    RecordVideo,
    UploadVideo,
    RecordAudio,
    UploadAudio,
    UploadDocument,
    FindLocation,
}

impl Into<String> for Action {
    fn into(self) -> String {
        let tmp = match self {
            Action::Typing => "Typing",
            Action::UploadPhoto => "UploadPhoto",
            Action::RecordVideo => "RecordVideo",
            Action::UploadVideo => "UploadVideo",
            Action::RecordAudio => "RecordVideo",
            Action::UploadAudio => "UploadAudio",
            Action::UploadDocument => "UploadDocument",
            Action::FindLocation => "FindLocation",
        };

        tmp.into()
    }
}

/// A simple method for testing your bot's auth token. Requires no parameters. Returns basic
/// information about the bot in form of a User object.
#[derive(TelegramFunction, Serialize)]
#[call = "getMe"]
#[answer = "User"]
#[function = "get_me"]
pub struct GetMe;

#[derive(TelegramFunction, Serialize)]
#[call = "getUpdates"]
#[answer = "Updates"]
#[function = "get_updates"]
pub struct GetUpdates {
    #[serde(skip_serializing_if = "Option::is_none")]
    offset: Option<Integer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<Integer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    timeout: Option<Integer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    allowed_updates: Option<Vec<String>>,
}

/// Use this method to send text messages. On success, the sent Message is returned.
#[derive(TelegramFunction, Serialize)]
#[call = "sendMessage"]
#[answer = "Message"]
#[function = "message"]
pub struct Message {
    chat_id: ChatID,
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    parse_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    disable_web_page_preview: Option<Boolean>,
    #[serde(skip_serializing_if = "Option::is_none")]
    disable_notificaton: Option<Boolean>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reply_to_message_id: Option<Integer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reply_markup: Option<NotImplemented>,
}

/// Use this method to get up to date information about the chat (current name of the user for
/// one-on-one conversations, current username of a user, group or channel, etc.). Returns a Chat
/// object on success.
#[derive(TelegramFunction, Serialize)]
#[call = "getChat"]
#[answer = "Chat"]
#[function = "get_chat"]
pub struct GetChat {
    chat_id: ChatID,
}

/// Use this method to get a list of administrators in a chat. On success, returns an Array of
/// ChatMember objects that contains information about all chat administrators except other bots.
/// If the chat is a group or a supergroup and no administrators were appointed, only the creator
/// will be returned.
#[derive(TelegramFunction, Serialize)]
#[call = "getChatAdministrators"]
#[answer = "Vector<objects::ChatMember>"]
#[function = "get_chat_administrators"]
pub struct GetChatAdministrators {
    chat_id: ChatID,
}

/// Use this method to get the number of members in a chat. Returns Int on success.
#[derive(TelegramFunction, Serialize)]
#[call = "getChatMembersCount"]
#[answer = "Integer"]
#[function = "get_chat_members_count"]
pub struct GetChatMemberCounts {
    chat_id: ChatID,
}

/// Use this method to get information about a member of a chat. Returns a ChatMember object on
/// success.
#[derive(TelegramFunction, Serialize)]
#[call = "getChatMember"]
#[answer = "ChatMember"]
#[function = "get_chat_member"]
pub struct GetChatMember {
    chat_id: ChatID,
    user_id: Integer,
}

/// Use this method to edit text and game messages sent by the bot or via the bot (for inline bots).
/// On success, if edited message is sent by the bot, the edited Message is returned,
/// otherwise True is returned.
#[derive(TelegramFunction, Serialize)]
#[call = "editMessageText"]
#[answer = "Message"]
#[function = "edit_message_text"]
pub struct EditMessageText {
    chat_id: ChatID,
    message_id: Integer,
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    parse_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    disable_web_page_preview: Option<Boolean>,
}

/// Use this method to delete a message.
/// A message can only be deleted if it was sent less than 48 hours ago.
/// Any such recently sent outgoing message may be deleted.
/// Additionally, if the bot is an administrator in a group chat, it can delete any message.
/// If the bot is an administrator in a supergroup,
/// it can delete messages from any other user and service messages about people joining or
/// leaving the group (other types of service messages may only be removed by the group creator).
/// In channels, bots can only remove their own messages. Returns True on success.
#[derive(TelegramFunction, Serialize)]
#[call = "deleteMessage"]
#[answer = "Boolean"]
#[function = "delete_message"]
pub struct DeleteMessage {
    chat_id: ChatID,
    message_id: Integer,
}

/// Use this method to send general files. On success, the sent Message is returned. Bots can
/// currently send files of any type of up to 50 MB in size, this limit may be changed in the
/// future.
#[derive(TelegramFunction, Serialize)]
#[call = "sendDocument"]
#[answer = "Message"]
#[function = "document"]
#[file_kind = "document"]
pub struct SendDocument {
    chat_id: Integer,
    document: File,
    #[serde(skip_serializing_if = "Option::is_none")]
    caption: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    disable_notification: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reply_to_message_id: Option<Integer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reply_markup: Option<NotImplemented>,
}
