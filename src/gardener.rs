use std::sync::{Arc, Mutex};

use tbot::{connectors::Connector, Bot};
use tokio::{
    self,
    time::{self, Duration},
};

use crate::data::Database;
use crate::BOT_ID;

pub fn start_pruning(bot: Bot<impl Connector>, db: Arc<Mutex<Database>>) {
    let mut interval = time::interval(Duration::from_secs(1 * 24 * 60 * 60));
    tokio::spawn(async move {
        loop {
            interval.tick().await;
            if let Err(e) = prune(&bot, &db).await {
                crate::print_error(e);
            }
        }
    });
}

async fn prune(
    bot: &Bot<impl Connector>,
    db: &Mutex<Database>,
) -> Result<(), tbot::errors::MethodCall> {
    let subscribers = db.lock().unwrap().all_subscribers();
    for subscriber in subscribers {
        let chat_id = tbot::types::chat::Id(subscriber);
        let chat = bot.get_chat(chat_id).call().await?;
        if chat.kind.is_group() || chat.kind.is_supergroup() || chat.kind.is_channel() {
            let me = bot
                .get_chat_member(chat_id, *BOT_ID.get().unwrap())
                .call()
                .await?;
            // Bots can only be added as administrators in channel,
            // so we don't need to check that.
            // And just ignore `can_post_messages` or `can_send_messages`
            if me.status.is_left() || me.status.is_kicked() {
                db.lock().unwrap().delete_subscriber(subscriber);
            }
        }
    }
    Ok(())
}
