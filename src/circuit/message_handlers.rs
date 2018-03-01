use circuit::MessageSender;
use messages::{MessageInstance, MessageType};
use std::collections::HashMap;
use failure::Fail;

type FilterFn = Box<Fn(&MessageInstance) -> bool + Send>;
type HandlerFn = Box<Fn(MessageInstance, &MessageSender) -> Result<(), Error> + Send>;

/// A message handler which handles all messages for which filter evaluates to
/// true.
struct FilterHandler {
    filter: FilterFn,
    handler: HandlerFn,
}

pub struct Handlers {
    type_handlers: HashMap<MessageType, HandlerFn>,
    filter_handlers: Vec<FilterHandler>,
}

impl Handlers {
    pub fn new() -> Handlers {
        Handlers {
            type_handlers: HashMap::new(),
            filter_handlers: Vec::new(),
        }
    }

    /// Register a handler for all messages of a specific message type.
    pub fn register_type(&mut self, m_type: MessageType, handler: HandlerFn) {
        self.type_handlers.insert(m_type, handler);
    }

    /// Register a handler for all messages for which the filter evaluates to
    /// true.
    pub fn register_filter(&mut self, filter: FilterFn, handler: HandlerFn) {
        self.filter_handlers.push(FilterHandler {
            filter: filter,
            handler: handler,
        });
    }

    pub(crate) fn handle(
        &self,
        msg: MessageInstance,
        msg_sender: &MessageSender,
    ) -> Result<(), Error> {
        if let Some(h) = self.type_handlers.get(&msg.message_type()) {
            h(msg, msg_sender)
        } else {
            for fh in &self.filter_handlers {
                if (fh.filter)(&msg) {
                    return (fh.handler)(msg, msg_sender);
                }
            }
            Err(Error {
                msg: msg,
                kind: ErrorKind::NoHandler,
            })
        }
    }
}

#[derive(Debug)]
pub struct Error {
    pub msg: MessageInstance,
    pub kind: ErrorKind,
}

#[derive(Debug)]
pub enum ErrorKind {
    NoHandler,
    // TODO: Make impossible and remove the variant.
    WrongHandler,
    Other(Box<Fail>),
}

impl Default for Handlers {
    fn default() -> Self {
        let mut handlers = Handlers::new();
        handlers.register_type(MessageType::StartPingCheck, Box::new(handle_ping_check));
        handlers
    }
}

fn handle_ping_check(msg: MessageInstance, circuit: &MessageSender) -> Result<(), Error> {
    use messages::all::{CompletePingCheck, CompletePingCheck_PingID};

    let start_ping_check = match msg {
        MessageInstance::StartPingCheck(m) => Ok(m),
        _ => Err(Error {
            msg: msg,
            kind: ErrorKind::WrongHandler,
        }),
    }?;
    let response = CompletePingCheck {
        ping_id: CompletePingCheck_PingID {
            ping_id: start_ping_check.ping_id.ping_id,
        },
    };
    circuit.send(response, false);
    Ok(())
}
