//! # Write a telegram bot in Rust
//!
//! This library allows you to write a Telegram Bot in Rust.
//! It's an almost complete wrapper for the Telegram Bot API and uses tokio-curl to send a request
//! to the Telegram server. Each Telegram function call returns a future and carries the actual bot
//! and the answer.
//! You can find all available functions in src/functions.rs. The crate telebot-derive implements
//! all required getter, setter and send functions automatically.
//!
//! # Example usage
//!
//! ```rust,no_run
//! extern crate telebot;
//! extern crate tokio_core;
//! extern crate futures;
//!
//! use telebot::bot;
//! use tokio_core::reactor::Core;
//! use futures::stream::Stream;
//!
//! // import all available functions
//! use telebot::functions::*;
//!
//! fn main() {
//!     // create a new event loop
//!     let mut lp = Core::new().unwrap();
//!
//!     // init the bot with the bot key and an update interval of 200ms
//!     let handle = lp.handle();
//!     let bot = lp.run(bot::RcBot::new(handle, "<TELEGRAM-BOT-TOKEN>")).unwrap()
//!         .update_interval(200);
//!
//!     // register a new command "reply" which replies all received messages
//!     let handle = bot.new_cmd("/reply")
//!     .and_then(|(bot, msg)| {
//!         let mut text = msg.text.unwrap().clone();
//!
//!         // when the text is empty send a dummy text
//!         if text.is_empty() {
//!             text = "<empty>".into();
//!         }
//!
//!         // construct a message and return a new future which will be resolved by tokio
//!         bot.message(msg.chat.id, text).send()
//!     });
//!
//!     // register the new command
//!     bot.register(handle);
//!
//!     // start the event loop
//!     bot.run(&mut lp).unwrap();
//! }
//! ```

#![feature(conservative_impl_trait)]
#![feature(custom_attribute)]
#![allow(unused_attributes)]

#[macro_use]
extern crate telebot_derive;

#[macro_use]
extern crate serde_derive;

extern crate serde;
extern crate serde_json;
extern crate curl;
extern crate futures;
extern crate tokio_core;
extern crate tokio_curl;

pub use bot::RcBot;
pub use error::Error;
pub use file::File;

pub mod bot;
pub mod error;
pub mod objects;
pub mod functions;
pub mod file;
