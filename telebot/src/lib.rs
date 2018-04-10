#![feature(custom_attribute)]
#![allow(unused_attributes)]

#[macro_use]
extern crate telebot_derive;

#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate log;
extern crate serde;
extern crate serde_json;
extern crate curl;
extern crate futures;
extern crate tokio_core;
extern crate tokio_curl;

pub use bot::RcBot;
pub use error::Error;

pub mod bot;
pub mod error;
pub mod objects;
pub mod functions;
