Telebot - Telegram Bot Library in Rust
======================================

[![Travis Build Status](https://travis-ci.org/bytesnake/telebot.svg)](https://travis-ci.org/bytesnake/telebot)
[![License MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/bytesnake/telebot/blob/master/LICENSE)
[![Crates.io](https://img.shields.io/crates/v/telebot.svg)](https://crates.io/crates/telebot)
[![doc.rs](https://docs.rs/telebot/badge.svg)](https://docs.rs/telebot)

This library allows you to write a Telegram Bot in Rust. It's an almost complete wrapper for the Telegram Bot API and uses tokio-curl to send requests to the Telegram server. Each Telegram function call returns a future which carries the actual bot and the answer.

## Usage
Add this to your `Cargo.toml`
``` toml
[dependencies]
telebot = "0.1.1"
```

## Example
``` rust
extern crate telebot;
extern crate tokio_core;
extern crate futures;

use telebot::bot;
use tokio_core::reactor::Core;                       
use futures::stream::Stream;
use futures::Future;
use std::fs::File;

// import all available functions
use telebot::functions::*;

fn main() {
    let mut lp = Core::new().unwrap();
    let bot = bot::RcBot::new(lp.handle(), "<TELEGRAM-BOT-TOKEN>")
        .update_interval(200);

    let handle = bot.new_cmd("/reply")
        .and_then(|(bot, msg)| {
            let mut text = msg.text.unwrap().clone();
            if text.is_empty() {
                text = "<empty>".into();
            }

            bot.message(msg.chat.id, text).send()
        });

    bot.register(handle);

    bot.run(&mut lp).unwrap();
}
```

## Find a Telegram function in the source code
All available functions are listed in src/functions.rs. For example consider sendLocation:
``` rust
/// Use this method to send point on the map. On success, the sent Message is returned.
#[derive(TelegramFunction, Serialize)]
#[call = "sendLocation"]
#[answer = "Message"]
#[function = "location"]
pub struct SendLocation {
    chat_id: u32,
    latitude: f32,
    longitude: f32,
#[serde(skip_serializing_if="Option::is_none")]
    disable_notification: Option<bool>,
#[serde(skip_serializing_if="Option::is_none")]                                                                                                             
    reply_to_message_id: Option<u32>,
#[serde(skip_serializing_if="Option::is_none")]
    reply_markup: Option<NotImplemented>
}
```

The field "function" defines the name of the function in the local API. Each optional field in the struct can be changed by calling the function with the name of the field.
So for example to send the location of Paris to chat 432432 silently: ` bot.location(432432, 48.8566, 2.3522).disable_notification(true).send() `
