//! The complete list of telegram types, copied from:
//! https://core.telegram.org/bots/api#available-types
//!
//! on each struct getter, setter and send function will be implemented

/// These objects are redefinitions of basic types. telebot-derive will scope every object in
/// answer, so we need to redefine them here.
pub type Boolean = bool;
pub type Integer = i64;
pub type Vector<T> = Vec<T>;
pub type NotImplemented = ::serde_json::Value;

/// This object represents a Telegram user or bot.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct User {
    pub id: Integer,
    pub first_name: String,
    pub last_name: Option<String>,
    pub username: Option<String>,
    pub language_code: Option<String>,
}

/// This object represents a chat.
#[derive(Deserialize, Debug)]
pub struct Chat {
    pub id: Integer,
    #[serde(rename = "type")]
    pub kind: String,
    pub title: Option<String>,
    pub username: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub all_members_are_administrators: Option<bool>,
}

/// This object represents one special entity in a text message. For example, hashtags, usernames,
/// URLs, etc.
#[derive(Deserialize, Debug)]
pub struct MessageEntity {
    #[serde(rename = "type")]
    pub kind: String,
    pub offset: Integer,
    pub length: Integer,
    pub url: Option<String>,
    pub user: Option<User>,
}

/// This object represents a message.
#[derive(Deserialize, Debug)]
pub struct Message {
    pub message_id: Integer,
    pub from: Option<User>,
    pub date: Integer,
    pub chat: Chat,
    pub forward_from: Option<User>,
    pub forward_from_chat: Option<Chat>,
    pub forward_from_message_id: Option<Integer>,
    pub forward_date: Option<Integer>,
    pub reply_to_message: Option<Box<Message>>,
    pub edit_date: Option<Integer>,
    pub text: Option<String>,
    pub entities: Option<Vec<MessageEntity>>,
    pub audio: Option<NotImplemented>,
    pub document: Option<NotImplemented>,
    pub game: Option<NotImplemented>,
    pub photo: Option<Vec<NotImplemented>>,
    pub sticker: Option<NotImplemented>,
    pub video: Option<NotImplemented>,
    pub voice: Option<NotImplemented>,
    pub video_note: Option<NotImplemented>,
    pub new_chat_members: Option<Vec<User>>,
    pub caption: Option<String>,
    pub contact: Option<NotImplemented>,
    pub location: Option<NotImplemented>,
    pub venue: Option<NotImplemented>,
    pub new_chat_member: Option<User>,
    pub left_chat_member: Option<User>,
    pub new_chat_title: Option<String>,
    pub new_chat_photo: Option<Vec<NotImplemented>>,
    pub delete_chat_photo: Option<bool>,
    pub group_chat_created: Option<bool>,
    pub supergroup_chat_created: Option<bool>,
    pub channel_chat_created: Option<bool>,
    pub migrate_to_chat_id: Option<Integer>,
    pub migrate_from_chat_id: Option<Integer>,
    pub pinned_message: Option<Box<Message>>,
    pub invoice: Option<NotImplemented>,
    pub successful_payment: Option<NotImplemented>,
}

#[derive(Deserialize, Debug)]
pub struct Updates(pub Vec<Update>);

#[derive(Deserialize, Debug)]
pub struct Update {
    pub update_id: Integer,
    pub message: Option<Message>,
    pub edited_message: Option<Message>,
    pub channel_post: Option<Message>,
    pub edited_channel_post: Option<Message>,
    pub inline_query: Option<NotImplemented>,
    pub chosen_inline_result: Option<NotImplemented>,
    pub callback_query: Option<NotImplemented>,
    pub shipping_query: Option<NotImplemented>,
    pub pre_checkout_query: Option<NotImplemented>,
}

/// This object contains information about one member of the chat.
#[derive(Deserialize, Debug)]
pub struct ChatMember {
    pub user: User,
    pub status: String,
}

/// Contains information about why a request was unsuccessfull.
#[derive(Deserialize, Debug)]
pub struct ResponseParameters {
    pub migrate_to_chat_id: Option<Integer>,
    pub retry_after: Option<Integer>,
}
