/*
TODO: Consider how something like this can be implemented.

use circuit::{message_handlers, MessageSender};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use futures::sync::oneshot;

struct LookupService<Res, MessageType> {
    message_sender: MessageSender,
    pending: Arc<Mutex<HashMap<Uuid, oneshot::Sender<Res>>>>,
    extractor: Box<Fn(MessageType) -> Result<Res, message_handlers::Error>>,
}
*/