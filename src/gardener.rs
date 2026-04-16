use std::sync::Arc;

use teloxide::{prelude::*, types::UserId, RequestError};
use tokio::{
    self,
    sync::Mutex,
    time::{self, Duration},
};

use crate::data::Database;
use crate::BOT_ID;

pub fn start_pruning(bot: Bot, db: Arc<Mutex<Database>>) {
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

async fn prune(bot: &Bot, db: &Mutex<Database>) -> Result<(), RequestError> {
    let subscribers = db.lock().await.all_subscribers();
    for subscriber in subscribers {
        let chat_id = ChatId(subscriber);
        let chat = bot.get_chat(chat_id).await?;
        if chat.is_group() || chat.is_supergroup() || chat.is_channel() {
            let me = bot
                .get_chat_member(chat_id, UserId(*BOT_ID.get().unwrap() as u64))
                .await?;
            // Bots can only be added as administrators in channel,
            // so we don't need to check that.
            // And just ignore `can_post_messages` or `can_send_messages`
            if me.kind.is_left() || me.kind.is_banned() {
                db.lock().await.delete_subscriber(subscriber);
            }
        }
    }
    Ok(())
}
