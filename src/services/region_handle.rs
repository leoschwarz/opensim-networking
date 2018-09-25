//! RegionHandle lookup.

use circuit::message_handlers;
use futures::sync::oneshot;
use grid_map::region_handle::RegionHandle;
use logging::Log;
use messages::{MessageInstance, MessageType};
use services::{CircuitDataHandle, Service};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use types::Uuid;

pub struct LookupService {
    circuit_data: CircuitDataHandle,
    pending: Arc<Mutex<HashMap<Uuid, oneshot::Sender<LookupResult>>>>,
}

impl Service for LookupService {
    fn register_service(
        handlers: &mut message_handlers::Handlers,
        circuit_data: CircuitDataHandle,
        _log: &Log,
    ) -> Self {
        let pending = Arc::new(Mutex::new(HashMap::new()));
        let pending2 = Arc::clone(&pending);

        let handler = Box::new(
            move |message: MessageInstance, _context: &message_handlers::HandlerContext| {
                match message {
                    MessageInstance::RegionIDAndHandleReply(msg) => {
                        let mut p = pending.lock().unwrap();
                        let uuid = msg.reply_block.region_id;
                        let handle = RegionHandle::from_handle(msg.reply_block.region_handle);

                        if let Some(sender) = p.remove(&uuid) {
                            let sender: oneshot::Sender<LookupResult> = sender;
                            sender
                                .send(LookupResult {
                                    uuid: uuid,
                                    handle: handle,
                                }).map_err(|_| message_handlers::Error {
                                    msg: MessageInstance::RegionIDAndHandleReply(msg),
                                    kind: message_handlers::ErrorKind::Other(Box::new(
                                        Error::ChannelClosed,
                                    )),
                                })
                        } else {
                            Err(message_handlers::Error {
                                msg: MessageInstance::RegionIDAndHandleReply(msg),
                                kind: message_handlers::ErrorKind::Other(Box::new(
                                    Error::NotRegistered,
                                )),
                            })
                        }
                    }
                    _ => Err(message_handlers::Error {
                        msg: message,
                        kind: message_handlers::ErrorKind::WrongHandler,
                    }),
                }
            },
        );
        handlers.register_type(MessageType::RegionIDAndHandleReply, handler);

        LookupService {
            circuit_data,
            pending: pending2,
        }
    }
}

#[derive(Debug, Fail)]
pub enum Error {
    #[fail(display = "Channel was closed prematurely.")]
    ChannelClosed,

    #[fail(display = "Handler was not registered (logic bug!)")]
    NotRegistered,
}

impl LookupService {
    pub fn lookup(&self, region_id: Uuid) -> oneshot::Receiver<LookupResult> {
        use messages::all::{RegionHandleRequest, RegionHandleRequest_RequestBlock};
        let msg = RegionHandleRequest {
            request_block: RegionHandleRequest_RequestBlock {
                region_id: region_id.clone(),
            },
        };

        // Register pending callback.
        let (sender, receiver) = oneshot::channel();
        {
            let mut pending = self.pending.lock().unwrap();
            pending.insert(region_id, sender);
        }

        // Send the request
        let _ = self.circuit_data.unwrap().message_sender.send(msg, true);
        receiver
    }
}

pub struct LookupResult {
    pub uuid: Uuid,
    pub handle: RegionHandle,
}
