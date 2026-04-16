use std::sync::Arc;

use tokio::sync::Mutex;

use super::{update_response, Command, Database, HandlerResult, MsgTarget, ReplyText};

pub async fn start(
    _db: Arc<Mutex<Database>>,
    cmd: Arc<Command>,
) -> HandlerResult {
    let target = &mut MsgTarget::new(cmd.chat.id, cmd.message_id);
    let msg = tr!("start_message");
    update_response(&cmd.bot, target, ReplyText::markdown(msg)).await?;
    Ok(())
}
