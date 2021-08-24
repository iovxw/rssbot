use std::sync::Arc;
use std::sync::Mutex;

use tbot::{contexts::Command, types::parameters};

use super::{update_response, Database, MsgTarget};

pub async fn start(
    _db: Arc<Mutex<Database>>,
    cmd: Arc<Command>,
) -> Result<(), tbot::errors::MethodCall> {
    let target = &mut MsgTarget::new(cmd.chat.id, cmd.message_id);
    let msg = tr!("start_message");
    update_response(&cmd.bot, target, parameters::Text::with_markdown(msg)).await?;
    Ok(())
}
