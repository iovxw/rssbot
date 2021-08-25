use std::sync::Arc;

use tbot::{contexts::Command, types::parameters};
use tokio::sync::Mutex;

use crate::data::Database;
use crate::messages::Escape;

use super::{check_channel_permission, update_response, MsgTarget};

pub async fn unsub(
    db: Arc<Mutex<Database>>,
    cmd: Arc<Command>,
) -> Result<(), tbot::errors::MethodCall> {
    let chat_id = cmd.chat.id;
    let text = &cmd.text.value;
    let args = text.split_whitespace().collect::<Vec<_>>();
    let mut target_id = chat_id;
    let target = &mut MsgTarget::new(chat_id, cmd.message_id);
    let feed_url;

    match &*args {
        [url] => feed_url = url,
        [channel, url] => {
            let channel_id = check_channel_permission(&cmd, channel, target).await?;
            if channel_id.is_none() {
                return Ok(());
            }
            target_id = channel_id.unwrap();
            feed_url = url;
        }
        [..] => {
            let msg = tr!("unsub_how_to_use");
            update_response(&cmd.bot, target, parameters::Text::with_plain(msg)).await?;
            return Ok(());
        }
    };
    let msg = if let Some(feed) = db.lock().await.unsubscribe(target_id.0, feed_url) {
        tr!(
            "unsubscription_succeeded",
            link = Escape(&feed.link),
            title = Escape(&feed.title)
        )
    } else {
        tr!("unsubscribed_from_rss").into()
    };
    update_response(&cmd.bot, target, parameters::Text::with_html(&msg)).await?;
    Ok(())
}
