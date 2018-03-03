//! Terrain data management.

use circuit::message_handlers;
use layer_data::{extract_land_patch, Patch};
use services::Service;
use std::cell::Cell;
use std::sync::{mpsc, Arc, Mutex};
use messages::{MessageInstance, MessageType};
use futures::Future;

pub struct Receivers {
    pub land_patches: mpsc::Receiver<Vec<Patch>>,
}

pub struct TerrainService {
    receivers: Cell<Option<Receivers>>,
}

impl Service for TerrainService {
    fn register_service(handlers: &mut message_handlers::Handlers) -> Self {
        let (patch_tx, patch_rx) = mpsc::channel();
        let patch_tx = Arc::new(Mutex::new(patch_tx));

        let handler = move |msg: MessageInstance, context: &message_handlers::HandlerContext| {
            let patch_tx = Arc::clone(&patch_tx);
            match msg {
                MessageInstance::LayerData(msg) => {
                    let patches = context.cpupool.spawn_fn(move || extract_land_patch(&msg));
                    let fut = patches
                        .map(move |patches| {
                            let tx = patch_tx.lock().unwrap();
                            tx.send(patches).unwrap();
                        })
                        .map_err(|_| {
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
