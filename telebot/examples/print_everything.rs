extern crate telebot;
extern crate tokio_core;
extern crate futures;

use telebot::RcBot;
use tokio_core::reactor::Core;
use futures::stream::Stream;
use std::env;
use futures::IntoFuture;

fn main() {
    // Create a new tokio core
    let mut lp = Core::new().unwrap();

    // Create the bot
    let bot = RcBot::new(lp.handle(), &env::var("TELEGRAM_BOT_KEY").unwrap()).update_interval(200);

    let stream = bot.get_stream()
        .and_then(|(_, msg)| {
            println!("Received: {:#?}", msg);

            Ok(())
        });

    // enter the main loop
    lp.run(stream.for_each(|_| Ok(())).into_future()).unwrap();
}
