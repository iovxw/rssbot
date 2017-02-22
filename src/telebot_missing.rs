use telebot;
use serde_json;
use futures::Future;

#[derive(Serialize)]
pub struct GetChatString {
    chat_id: String,
}

pub struct WrapperGetChatString {
    bot: telebot::RcBot,
    inner: GetChatString,
}

impl WrapperGetChatString {
    pub fn send<'a>(self) -> impl Future<Item = (telebot::RcBot, telebot::objects::Chat), Error = telebot::Error> + 'a {
        let msg = serde_json::to_string(&self.inner).unwrap();
        self.bot
            .inner
            .fetch_json("getChat", &msg)
            .map(move |x| (self.bot.clone(), serde_json::from_str::<telebot::objects::Chat>(&x).unwrap()))
    }
}

#[derive(Serialize)]
pub struct EditMessageText {
    chat_id: i64,
    message_id: i64,
    text: String,
    #[serde(skip_serializing_if="Option::is_none")]
    parse_mode: Option<String>,
    #[serde(skip_serializing_if="Option::is_none")]
    disable_web_page_preview: Option<bool>,
}

pub fn get_chat_string(bot: &telebot::RcBot, chat: String) -> WrapperGetChatString {
    WrapperGetChatString {
        bot: bot.clone(),
        inner: GetChatString { chat_id: chat },
    }
}

pub struct WrapperEditMessageText {
    bot: telebot::RcBot,
    inner: EditMessageText,
}

impl WrapperEditMessageText {
    pub fn send<'a>(self) -> impl Future<Item = (telebot::RcBot, telebot::objects::Message), Error = telebot::Error> + 'a {
        let msg = serde_json::to_string(&self.inner).unwrap();
        self.bot
            .inner
            .fetch_json("editMessageText", &msg)
            .map(move |x| (self.bot.clone(), serde_json::from_str::<telebot::objects::Message>(&x).unwrap()))
    }

    pub fn parse_mode<S>(mut self, val: S) -> Self
        where S: Into<String>
    {
        self.inner.parse_mode = Some(val.into());
        self
    }

    pub fn disable_web_page_preview<S>(mut self, val: S) -> Self
        where S: Into<bool>
    {
        self.inner.disable_web_page_preview = Some(val.into());
        self
    }
}

pub fn edit_message_text(bot: &telebot::RcBot, chat_id: i64, message_id: i64, text: String) -> WrapperEditMessageText {
    WrapperEditMessageText {
        bot: bot.clone(),
        inner: EditMessageText {
            chat_id: chat_id,
            message_id: message_id,
            text: text,
            parse_mode: None,
            disable_web_page_preview: None,
        },
    }
}
