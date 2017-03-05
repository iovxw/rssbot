extern crate telebot;
extern crate tokio_core;
extern crate futures;

use telebot::{RcBot, Error};
use tokio_core::reactor::Core;
use futures::stream::Stream;
use futures::Future;
use std::env;

// import all available functions
use telebot::functions::*;

fn main() {
    // Create a new tokio core
    let mut lp = Core::new().unwrap();

    // Create the bot
    let bot = RcBot::new(lp.handle(), &env::var("TELEGRAM_BOT_KEY").unwrap()).update_interval(200);

    // Register a location command which will send a location to requests like /location 2.321 12.32
    enum LocationErr {
        Telegram(Error),
        WrongLocationFormat,
    }

    let handle = bot.new_cmd("/location")
        .then(|result| {
            let (bot, mut msg) = result.expect("Strange telegram error!");

            let mut pos = msg.text
                .take()
                .unwrap()
                .split_whitespace()
                .take(2)
                .map(|x| x.parse::<f32>())
                .collect::<Vec<Result<f32, _>>>();

            if let Some(Ok(longitude)) = pos.pop() {
                if let Some(Ok(altitude)) = pos.pop() {
                    return Ok((bot, msg, longitude, altitude));
                }
            }

            return Err((bot, msg, LocationErr::WrongLocationFormat));
        })
        .and_then(|(bot, msg, long, alt)| bot.location(msg.chat.id, long, alt).send().map_err(|err| (bot, msg, LocationErr::Telegram(err))))
        .or_else(|(bot, msg, err)| {
            let text = {
                match err {
                    LocationErr::Telegram(err) => format!("Telegram error: {:?}", err),
                    LocationErr::WrongLocationFormat => "Couldn't parse the location!".into(),
                }
            };

            bot.message(msg.chat.id, text).send()
        });

    bot.register(handle);

    // Register a get_my_photo command which will send the own profile photo to the chat
    enum PhotoErr {
        Telegram(Error),
        NoPhoto,
    }

    let handle2 = bot.new_cmd("/get_my_photo")
        .then(|result| {
            let (bot, msg) = result.expect("Strange telegram error!");

            let user_id = msg.from.clone().unwrap().id;

            bot.get_user_profile_photos(user_id)
                .limit(1u32)
                .send()
                .then(|result| match result {
                    Ok((bot, photos)) => {
                        if photos.total_count == 0 {
                            return Err((bot, msg, PhotoErr::NoPhoto));
                        }

                        return Ok((bot, msg, photos.photos[0][0].clone().file_id));
                    }
                    Err(err) => Err((bot, msg, PhotoErr::Telegram(err))),
                })
        })
        .and_then(|(bot, msg, file_id)| bot.photo(msg.chat.id).file_id(file_id).send().map_err(|err| (bot, msg, PhotoErr::Telegram(err))))
        .or_else(|(bot, msg, err)| {
            let text = match err {
                PhotoErr::Telegram(err) => format!("Telegram Error: {:?}", err),
                PhotoErr::NoPhoto => "No photo exists!".into(),
            };

            bot.message(msg.chat.id, text).send()
        });

    bot.register(handle2);

    // enter the main loop
    bot.run(&mut lp).unwrap();
}
