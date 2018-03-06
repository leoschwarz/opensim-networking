//! Terrain data management.

use circuit::message_handlers;
use crossbeam_channel;
use layer_data::{extract_land_patch, Patch};
use services::Service;
use std::cell::Cell;
use std::sync::{Arc, Mutex};
use messages::{MessageInstance, MessageType};
use futures::Future;
use logging::{Log, Logger};

pub struct Receivers {
    pub land_patches: crossbeam_channel::Receiver<Vec<Patch>>,
}

pub struct TerrainService {
    receivers: Cell<Option<Receivers>>,
}

impl Service for TerrainService {
    fn register_service(handlers: &mut message_handlers::Handlers, log: &Log) -> Self {
        let (patch_tx, patch_rx) = crossbeam_channel::bounded(100);
        let patch_tx = Arc::new(Mutex::new(patch_tx));
        let logger = Arc::new(Logger::root(log.clone(), o!("service" => "TerrainService")));

        let handler = move |msg: MessageInstance, context: &message_handlers::HandlerContext| {
            let patch_tx = Arc::clone(&patch_tx);
            match msg {
                MessageInstance::LayerData(msg) => {
                    debug!(logger, "Received new layer data msg");
                    let logger2 = Arc::clone(&logger);
                    let logger3 = Arc::clone(&logger);
                    let patches = context.cpupool.spawn_fn(move || {
                        extract_land_patch(&msg).map(|patches| {
                            debug!(logger2, "Decoding layer data ok.");
                            let tx = patch_tx.lock().unwrap();
                            tx.send(patches).unwrap();
                        })
                    });
                    let fut = patches.map_err(move |e| {
                        debug!(logger3, "Decoding layer data failed: {:?}", e);
                        // TODO
                        ()
                    });

                    context.reactor.spawn(|_handle| fut);

                    Ok(())
                }
                _ => Err(message_handlers::Error {
                    msg: msg,
                    kind: message_handlers::ErrorKind::WrongHandler,
                }),
            }
        };
        handlers.register_type(MessageType::LayerData, Box::new(handler));

        let receivers = Receivers {
            land_patches: patch_rx,
        };

        TerrainService {
            receivers: Cell::new(Some(receivers)),
        }
    }
}

impl TerrainService {
    /// Returns the Receivers on the first invocation, afterwards only None
    /// will be returned.
    pub fn receivers(&self) -> Option<Receivers> {
        self.receivers.replace(None)
    }
}
