use circuit::ack_manager::AckManagerTx;
use circuit::status::SendMessage;
use messages::{MessageInstance, MessageType};
use std::collections::HashMap;
use std::error::Error;
use std::ops::{Deref, DerefMut};

type Handler = Box<Fn(MessageInstance, &MessageSender) -> Result<(), MessageHandlerError> + Send>;
type Handlers = HashMap<MessageType, Handler>;

pub struct MessageHandlers {
    inner: Handlers,
}

impl Deref for MessageHandlers {
    type Target = Handlers;

    fn deref(&self) -> &Handlers {
        &self.inner
    }
}

impl DerefMut for MessageHandlers {
    fn deref_mut(&mut self) -> &mut Handlers {
        &mut self.inner
    }
}

impl Default for MessageHandlers {
    fn default() -> Self {
        let mut handlers = Handlers::new();

        handlers.insert(MessageType::StartPingCheck, Box::new(handle_ping_check));

        MessageHandlers { inner: handlers }
    }
}

fn handle_ping_check(
    msg: MessageInstance,
    circuit: &MessageSender,
) -> Result<(), MessageHandlerError> {
    use messages::all::{CompletePingCheck, CompletePingCheck_PingID};

    let start_ping_check = match msg {
        MessageInstance::StartPingCheck(m) => Ok(m),
        _ => Err(MessageHandlerError::WrongHandler),
    }?;
    let response = CompletePingCheck {
        ping_id: CompletePingCheck_PingID {
            ping_id: start_ping_check.ping_id.ping_id,
        },
    };
    circuit.send(response, false);
    Ok(())
}

// TODO: Differentiate between recoverable and non-recoverable errors.
#[derive(Debug)]
pub enum MessageHandlerError {
    /// Message handler does not know how to handle the message instance.
    // TODO: Make this impossible?
    WrongHandler,
    Other(Box<Error>),
}

/// Can be used by MessageHandler instances to send a message through the
/// Circuit.
pub struct MessageSender {
    pub(crate) ackmgr_tx: AckManagerTx,
}

impl MessageSender {
    /// See: `Ciruit::send()` for more information.
    pub fn send<M: Into<MessageInstance>>(&self, msg: M, reliable: bool) -> SendMessage {
        self.ackmgr_tx.send_msg(msg.into(), reliable)
    }
}
