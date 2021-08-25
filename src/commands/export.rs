use std::sync::Arc;

use tbot::{
    contexts::Command,
    types::{input_file, parameters},
};
use tokio::sync::Mutex;

use crate::data::Database;
use crate::opml::into_opml;

use super::{check_channel_permission, update_response, MsgTarget};

pub async fn export(
    db: Arc<Mutex<Database>>,
    cmd: Arc<Command>,
) -> Result<(), tbot::errors::MethodCall> {
    let chat_id = cmd.chat.id;
    let channel = &cmd.text.value;
    let mut target_id = chat_id;
    let target = &mut MsgTarget::new(chat_id, cmd.message_id);

    if !channel.is_empty() {
        let channel_id = check_channel_permission(&cmd, channel, target).await?;
        if channel_id.is_none() {
            return Ok(());
        }
        target_id = channel_id.unwrap();
    }

    let feeds = db.lock().await.subscribed_feeds(target_id.0);
    if feeds.is_none() {
        update_response(
            &cmd.bot,
            target,
            parameters::Text::with_plain(tr!("subscription_list_empty")),
        )
        .await?;
        return Ok(());
    }
    let opml = into_opml(feeds.unwrap());

    cmd.bot
        .send_document(
            chat_id,
            input_file::Document::with_bytes("feeds.opml", opml.as_bytes()),
        )
        .in_reply_to(cmd.message_id)
        .call()
        .await?;
    Ok(())
}
