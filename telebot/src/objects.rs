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
}

/// This object represents a chat.
#[derive(Deserialize, Debug)]
pub struct Chat {
    pub id: Integer,
    #[serde(rename="type")]
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
    #[serde(rename="type")]
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
    pub audio: Option<Audio>,
    pub document: Option<Document>,
    pub game: Option<NotImplemented>,
    pub photo: Option<Vec<PhotoSize>>,
    pub sticker: Option<Sticker>,
    pub video: Option<Video>,
    pub voice: Option<Voice>,
    pub caption: Option<String>,
    pub contact: Option<Contact>,
    pub location: Option<Location>,
    pub venue: Option<Venue>,
    pub new_chat_member: Option<User>,
    pub left_chat_member: Option<User>,
    pub new_chat_title: Option<String>,
    pub new_chat_photo: Option<Vec<PhotoSize>>,
    pub delete_chat_photo: Option<bool>,
    pub group_chat_created: Option<bool>,
    pub supergroup_chat_created: Option<bool>,
    pub channel_chat_created: Option<bool>,
    pub migrate_to_chat_id: Option<Integer>,
    pub migrate_from_chat_id: Option<Integer>,
    pub pinned_message: Option<Box<Message>>,
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
    pub inline_query: Option<InlineQuery>,
    pub chosen_inline_result: Option<()>,
    pub callback_query: Option<()>,
}

/// This object represents one size of a photo or a file / sticker thumbnail.
#[derive(Deserialize, Debug, Clone)]
pub struct PhotoSize {
    pub file_id: String,
    pub width: Integer,
    pub height: Integer,
    pub file_size: Option<Integer>,
}

/// This object represents an audio file to be treated as music by the Telegram clients.
#[derive(Deserialize, Debug)]
pub struct Audio {
    pub file_id: String,
    pub duration: Integer,
    pub performer: Option<String>,
    pub title: Option<String>,
    pub mime_type: Option<String>,
    pub file_size: Option<Integer>,
}

/// This object represents a general file (as opposed to photos, voice messages and audio files).
#[derive(Deserialize, Debug)]
pub struct Document {
    pub file_id: String,
    pub thumb: Option<PhotoSize>,
    pub file_name: Option<String>,
    pub mime_type: Option<String>,
    pub file_size: Option<Integer>,
}

/// This object represents a sticker.
#[derive(Deserialize, Debug)]
pub struct Sticker {
    pub file_id: String,
    pub width: Integer,
    pub height: Integer,
    pub thumb: Option<PhotoSize>,
    pub emoji: Option<String>,
    pub file_size: Option<Integer>,
}

/// This object represents a video file.
#[derive(Deserialize, Debug)]
pub struct Video {
    pub file_id: String,
    pub width: Integer,
    pub height: Integer,
    pub duration: Integer,
    pub thumb: Option<PhotoSize>,
    pub mime_type: Option<String>,
    pub file_size: Option<Integer>,
}

/// This object represents a voice note.
#[derive(Deserialize, Debug)]
pub struct Voice {
    pub file_id: String,
    pub duration: Integer,
    pub mime_type: Option<String>,
    pub file_size: Option<Integer>,
}

/// This object represents a phone contact.
#[derive(Deserialize, Debug)]
pub struct Contact {
    pub phone_number: String,
    pub first_name: String,
    pub last_name: Option<String>,
    pub user_id: Option<Integer>,
}

/// This object represents a point on the map.
#[derive(Serialize, Deserialize, Debug)]
pub struct Location {
    pub longitude: f32,
    pub latitude: f32,
}

/// This object represents a venue.
#[derive(Deserialize, Debug)]
pub struct Venue {
    pub location: Location,
    pub title: String,
    pub address: String,
    pub foursquare_id: Option<String>,
}

/// This object represent a user's profile pictures.
#[derive(Deserialize, Debug)]
pub struct UserProfilePhotos {
    pub total_count: Integer,
    pub photos: Vec<Vec<PhotoSize>>,
}

/// This object represents a file ready to be downloaded. The file can be downloaded via the link
/// https://api.telegram.org/file/bot<token>/<file_path>. It is guaranteed that the link will be
/// valid for at least 1 hour. When the link expires, a new one can be requested by calling
/// getFile.
#[derive(Deserialize, Debug)]
pub struct File {
    pub file_id: String,
    pub file_size: Option<Integer>,
    pub file_path: Option<String>,
}

/// This object represents a custom keyboard with reply options (see Introduction to bots for
/// details and examples).
#[derive(Deserialize, Debug)]
pub struct ReplyKeyboardMarkup {
    pub keyboard: Vec<KeyboardButton>,
    pub resize_keyboard: Option<bool>,
    pub one_time_keyboard: Option<bool>,
    pub selective: Option<bool>,
}

/// This object represents one button of the reply keyboard. For simple text buttons String can be
/// used instead of this object to specify text of the button. Optional fields are mutually
/// exclusive.
#[derive(Deserialize, Debug)]
pub struct KeyboardButton {
    pub text: String,
    pub request_contact: Option<bool>,
    pub request_location: Option<bool>,
}

/// Upon receiving a message with this object, Telegram clients will remove the current custom
/// keyboard and display the default letter-keyboard. By default, custom keyboards are displayed
/// until a new keyboard is sent by a bot. An exception is made for one-time keyboards that are
/// hidden immediately after the user presses a button (see ReplyKeyboardMarkup).
#[derive(Deserialize, Debug)]
pub struct ReplyKeyboardRemove {
    pub remove_keyboard: bool,
    pub selective: Option<bool>,
}

/// This object represents an inline keyboard that appears right next to the message it belongs to.
#[derive(Serialize, Deserialize, Debug)]
pub struct InlineKeyboardMarkup {
    pub inline_keyboard: Vec<InlineKeyboardButton>,
}

/// This object represents one button of an inline keyboard. You must use exactly one of the
/// optional fields.
#[derive(Serialize, Deserialize, Debug)]
pub struct InlineKeyboardButton {
    pub text: String,
    pub url: Option<String>,
    pub callback_data: Option<String>,
    pub switch_inline_query: Option<String>,
    pub switch_inline_query_current_chat: Option<String>,
    pub callback_game: Option<CallbackGame>,
}

/// This object represents an incoming callback query from a callback button in an inline keyboard.
/// If the button that originated the query was attached to a message sent by the bot, the field
/// message will be present. If the button was attached to a message sent via the bot (in inline
/// mode), the field inline_message_id will be present. Exactly one of the fields data or
/// game_short_name will be present.
#[derive(Deserialize, Debug)]
pub struct CallbackQuery {
    pub id: String,
    pub from: User,
    pub message: Option<Message>,
    pub inline_message_id: Option<String>,
    pub chat_instance: Option<String>,
    pub data: Option<String>,
    pub game_short_name: Option<String>,
}

/// Upon receiving a message with this object, Telegram clients will display a reply interface to
/// the user (act as if the user has selected the bot‘s message and tapped ’Reply'). This can be
/// extremely useful if you want to create user-friendly step-by-step interfaces without having to
/// sacrifice privacy mode.
#[derive(Deserialize, Debug)]
pub struct ForceReply {
    pub force_reply: bool,
    pub selective: Option<bool>,
}

/// This object contains information about one member of the chat.
#[derive(Deserialize, Debug)]
pub struct ChatMember {
    pub user: User,
    pub status: String,
}

/// Contains information about why a request was unsuccessfull.
#[derive(Deserialize, Debug)]
pub struct ResponseParameter {
    pub migrate_to_chat_id: Option<Integer>,
    pub retry_after: Option<Integer>,
}

/// A placeholder, currently holds no information. Use BotFather to set up your game.
#[derive(Serialize, Deserialize, Debug)]
pub struct CallbackGame;

///This object represents an incoming inline query. When the user sends an empty query, youur bot
///could return some default or  trending results.
#[derive(Deserialize,Debug)]
pub struct InlineQuery {
    pub id: String,
    pub from: User,
    pub location: Option<Location>,
    pub query: String,
    pub offset: String,
}

/*#[derive(Serialize)]
pub enum InlineQueryResult {
    CachedAudio(InlineQueryResultCachedAudio),
    CachedDocument(InlineQueryResultCachedDocument),
    CachedGif(InlineQueryResultCachedGif),
    CachedMpeg4Gif(InlineQueryResultCachedMpeg4Gif),
    CachedPhoto(InlineQueryResultCachedPhoto),
    CachedSticker(InlineQueryResultCachedSticker),
    CachedVideo(InlineQueryResultCachedVideo),
    CachedVoice(InlineQueryResultCachedVoice),
    Article(InlineQueryResultArticle),
    Audio(InlineQueryResultAudio),
    Contact(InlineQueryResultContact),
    Game(InlineQueryResultGame),
    Document(InlineQueryResultDocument),
    Gif(InlineQueryResultGif),
    Location(InlineQueryResultLocation),
    Mpeg4Gif(InlineQueryResultMpeg4Gif),
    Photo(InlineQueryResultPhoto),
    Venue(InlineQueryResultVenue),
    Video(InlineQueryResultVideo),
    Voice(InlineQueryResultVoice)
}*/
/*
#[derive(setter, Serialize)]
#[query="Article"]
pub struct InlineQueryResultArticle {
    #[serde(rename="type")]
    pub kind: String,
    pub id: String,
    pub title: String,
    pub input_message_content: Box<Serialize>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub reply_markup: Option<InlineKeyboardMarkup>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub hide_url: Option<Boolean>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub thumb_url: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub thumb_width: Option<Integer>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub thumb_height: Option<Integer>,
}

#[derive(setter, Serialize)]
#[query="Photo"]
pub struct InlineQueryResultPhoto {
    #[serde(rename="type")]
    pub kind: String,
    pub id: String,
    pub photo_url: String,
    pub thumb_url: String,
    #[serde(skip_serializing_if="Option::is_none")]
    pub photo_width: Option<Integer>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub photo_height: Option<Integer>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub caption: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub reply_markup: Option<InlineKeyboardMarkup>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub input_message_content: Option<Box<Serialize>>,
}

#[derive(setter, Serialize)]
#[query="Gif"]
pub struct InlineQueryResultGif {
    #[serde(rename="type")]
    pub kind: String,
    pub id: String,
    pub gif_url: String,
    pub thumb_url: String,
    #[serde(skip_serializing_if="Option::is_none")]
    pub gif_width: Option<Integer>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub gif_height: Option<Integer>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub caption: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub reply_markup: Option<InlineKeyboardMarkup>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub input_message_content: Option<Box<Serialize>>,
}

#[derive(setter, Serialize)]
#[query="Mpeg4Gif"]
pub struct InlineQueryResultMpeg4Gif {
    #[serde(rename="type")]
    pub kind: String,
    pub id: String,
    pub mpeg4_url: String,
    pub thumb_url: String,
    #[serde(skip_serializing_if="Option::is_none")]
    pub mpeg4_width: Option<Integer>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub mpeg4_height: Option<Integer>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub caption: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub reply_markup: Option<InlineKeyboardMarkup>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub input_message_content: Option<Box<Serialize>>,
}

#[derive(setter, Serialize)]
#[query="Video"]
pub struct InlineQueryResultVideo {
    #[serde(rename="type")]
    pub kind: String,
    pub id: String,
    pub video_url: String,
    pub mime_type: String,
    pub thumb_url: String,
    pub title: String,
    #[serde(skip_serializing_if="Option::is_none")]
    pub caption: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub video_width: Option<Integer>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub video_height: Option<Integer>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub video_duration: Option<Integer>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub reply_markup: Option<InlineKeyboardMarkup>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub input_message_content: Option<Box<Serialize>>,
}

#[derive(setter, Serialize)]
#[query="Audio"]
pub struct InlineQueryResultAudio {
    #[serde(rename="type")]
    pub kind: String,
    pub id: String,
    pub audio_url: String,
    pub title: String,
    #[serde(skip_serializing_if="Option::is_none")]
    pub caption: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub performer: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub audio_duration: Option<Integer>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub reply_markup: Option<InlineKeyboardMarkup>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub input_message_content: Option<Box<Serialize>>,
}

#[derive(setter, Serialize)]
#[query="Voice"]
pub struct InlineQueryResultVoice {
    #[serde(rename="type")]
    pub kind: String,
    pub id: String,
    pub voice_url: String,
    pub title: String,
    #[serde(skip_serializing_if="Option::is_none")]
    pub caption: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub voice_duration: Option<Integer>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub reply_markup: Option<InlineKeyboardMarkup>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub input_message_content: Option<Box<Serialize>>,
}

#[derive(setter, Serialize)]
#[query="Document"]
pub struct InlineQueryResultDocument {
    #[serde(rename="type")]
    pub kind: String,
    pub id: String,
    pub title: String,
    pub document_url: String,
    pub mime_type: String,
    #[serde(skip_serializing_if="Option::is_none")]
    pub caption: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub reply_markup: Option<InlineKeyboardMarkup>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub input_message_content: Option<Box<Serialize>>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub thumb_url: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub thumb_width: Option<Integer>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub thumb_height: Option<Integer>,
}

#[derive(setter, Serialize)]
#[query="Location"]
pub struct InlineQueryResultLocation {
    #[serde(rename="type")]
    pub kind: String,
    pub id: String,
    pub latitude: f64,
    pub longitude: f64,
    pub title: String,
    #[serde(skip_serializing_if="Option::is_none")]
    pub reply_markup: Option<InlineKeyboardMarkup>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub input_message_content: Option<Box<Serialize>>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub thumb_url: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub thumb_width: Option<Integer>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub thumb_height: Option<Integer>,
}

#[derive(setter, Serialize)]
#[query="Venue"]
pub struct InlineQueryResultVenue {
    #[serde(rename="type")]
    pub kind: String,
    pub id: String,
    pub latitude: f64,
    pub longitude: f64,
    pub title: String,
    pub address: String,
    pub foursquare_id: String,
    #[serde(skip_serializing_if="Option::is_none")]
    pub reply_markup: Option<InlineKeyboardMarkup>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub input_message_content: Option<Box<Serialize>>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub thumb_url: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub thumb_width: Option<Integer>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub thumb_height: Option<Integer>,
}

#[derive(setter, Serialize)]
#[query="Contact"]
pub struct InlineQueryResultContact {
    #[serde(rename="type")]
    pub kind: String,
    pub id: String,
    pub phone_number: String,
    pub first_name: String,
    #[serde(skip_serializing_if="Option::is_none")]
    pub last_name: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub reply_markup: Option<InlineKeyboardMarkup>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub input_message_content: Option<Box<Serialize>>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub thumb_url: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub thumb_width: Option<Integer>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub thumb_height: Option<Integer>,
}

#[derive(setter, Serialize)]
#[query="Game"]
pub struct InlineQueryResultGame {
    #[serde(rename="type")]
    pub kind: String,
    pub id: String,
    pub game_short_name: String,
    #[serde(skip_serializing_if="Option::is_none")]
    pub reply_markup: Option<InlineKeyboardMarkup>,
}

#[derive(setter,Serialize)]
#[query="CachedPhoto"]
pub struct InlineQueryResultCachedPhoto {
    #[serde(rename="type")]
    pub kind: String,
    pub id: String,
    pub photo_file_id: String,
    #[serde(skip_serializing_if="Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub caption: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub reply_markup: Option<InlineKeyboardMarkup>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub input_message_content: Option<Box<Serialize>>,
}

#[derive(setter, Serialize)]
#[query="CachedGif"]
pub struct InlineQueryResultCachedGif {
    #[serde(rename="type")]
    pub kind: String,
    pub id: String,
    pub gif_file_id: String,
    #[serde(skip_serializing_if="Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub caption: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub reply_markup: Option<InlineKeyboardMarkup>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub input_message_content: Option<Box<Serialize>>,
}

#[derive(setter, Serialize)]
#[query="CachedMpeg4Gif"]
pub struct InlineQueryResultCachedMpeg4Gif {
    #[serde(rename="type")]
    pub kind: String,
    pub id: String,
    pub mpeg4_file_id: String,
    #[serde(skip_serializing_if="Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub caption: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub reply_markup: Option<InlineKeyboardMarkup>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub input_message_content: Option<Box<Serialize>>,
}

#[derive(setter, Serialize)]
#[query="CachedSticker"]
pub struct InlineQueryResultCachedSticker {
    #[serde(rename="type")]
    pub kind: String,
    pub id: String,
    pub sticker_file_id: String,
    #[serde(skip_serializing_if="Option::is_none")]
    pub reply_markup: Option<InlineKeyboardMarkup>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub input_message_content: Option<Box<Serialize>>,
}

#[derive(setter, Serialize)]
#[query="CachedDocument"]
pub struct InlineQueryResultCachedDocument {
    #[serde(rename="type")]
    pub kind: String,
    pub id: String,
    pub title: String,
    pub document_file_id: String,
    #[serde(skip_serializing_if="Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub caption: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub reply_markup: Option<InlineKeyboardMarkup>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub input_message_content: Option<Box<Serialize>>,
}

#[derive(setter, Serialize)]
#[query="CachedVideo"]
pub struct InlineQueryResultCachedVideo {
    #[serde(rename="type")]
    pub kind: String,
    pub id: String,
    pub video_file_id: String,
    pub title: String,
    #[serde(skip_serializing_if="Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub caption: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub reply_markup: Option<InlineKeyboardMarkup>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub input_message_content: Option<Box<Serialize>>,
}

#[derive(setter, Serialize)]
#[query="CachedVoice"]
pub struct InlineQueryResultCachedVoice {
    #[serde(rename="type")]
    pub kind: String,
    pub id: String,
    pub voice_file_id: String,
    pub title: String,
    #[serde(skip_serializing_if="Option::is_none")]
    pub caption: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub reply_markup: Option<InlineKeyboardMarkup>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub input_message_content: Option<Box<Serialize>>,
}

#[derive(setter, Serialize)]
#[query="CachedAudio"]
pub struct InlineQueryResultCachedAudio {
    #[serde(rename="type")]
    pub kind: String,
    pub id: String,
    pub audio_file_id: String,
    #[serde(skip_serializing_if="Option::is_none")]
    pub caption: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub reply_markup: Option<InlineKeyboardMarkup>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub input_message_content: Option<Box<Serialize>>,
}

pub mod input_message_content {
    use super::Boolean;

    #[derive(setter, Serialize, Deserialize, Debug)]
    pub struct Text {
        pub message_text: String,
        #[serde(skip_serializing_if="Option::is_none")]
        pub parse_mode: Option<String>,
        #[serde(skip_serializing_if="Option::is_none")]
        pub disable_web_page_preview: Option<Boolean>,
    }

    #[derive(setter, Serialize, Deserialize, Debug)]
    pub struct Location {
        pub latitude: f64,
        pub longitude: f64,
    }

    #[derive(setter,Serialize, Deserialize, Debug)]
    pub struct Venue {
        pub latitude: f64,
        pub longitude: f64,
        pub title: String,
        pub address: String,
        #[serde(skip_serializing_if="Option::is_none")]
        pub foursquare_id: Option<String>,
    }

    #[derive(setter, Serialize, Deserialize, Debug)]
    pub struct Contact {
        pub phone_number: String,
        pub first_name: String,
        #[serde(skip_serializing_if="Option::is_none")]
        pub last_name: Option<String>,
    }
}

#[derive(setter,Serialize, Deserialize, Debug)]
pub struct ChosenInlineResult {
    pub result_id: String,
    pub from: User,
    pub offset: String,
    #[serde(skip_serializing_if="Option::is_none")]
    pub location: Option<Location>,
    #[serde(skip_serializing_if="Option::is_none")]
    pub inline_message_id: Option<String>,
}
*/
