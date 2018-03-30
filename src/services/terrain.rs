//! Terrain data management.

use circuit::message_handlers;
use crossbeam_channel;
use futures::Future;
use layer_data::{extract_land_patch, Patch};
use logging::{Log, Logger};
use messages::{MessageInstance, MessageType};
use services::Service;
use std::cell::Cell;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use types::Uuid;

#[derive(Debug, Fail)]
pub enum ReceiversError {
    #[fail(display = "Attempted to register Receivers twice for region: {}", 0)]
    RegisterTwice(Uuid),

    #[fail(display = "Receiver for TerrainService already moved, for region: {}", 0)]
    ReceiverMoved(Uuid),
}

/// Manages the receivers of a set of regions.
///
/// Receivers will be removed when iterating received patches,
/// when found to be disconnected.
pub struct Receivers {
    receivers: HashMap<Uuid, Receiver>,
}

impl Receivers {
    pub fn new() -> Self {
        Receivers {
            receivers: HashMap::new(),
        }
    }

    /// Register a terrain service to this receivers manager.
    ///
    /// TODO: In the future region_id might be optional if this data can be
    /// passed       to tReceiversError
    pub fn register(
        &mut self,
        region_id: Uuid,
        terrain_service: &TerrainService,
    ) -> Result<(), ReceiversError> {
        if self.receivers.contains_key(&region_id) {
            return Err(ReceiversError::RegisterTwice(region_id));
        }

        if let Some(receiver) = terrain_service.receiver.replace(None) {
            self.receivers.insert(region_id, receiver);
            Ok(())
        } else {
            Err(ReceiversError::ReceiverMoved(region_id))
        }
    }

    /// Iterates through all currently available patches and invokes
    /// `handler` for each of them.
    pub fn receive_patches<F: Fn(Patch)>(&mut self, handler: F) {
        let mut disconnected = Vec::new();

        for (region_id, receiver) in self.receivers.iter_mut() {
            let lock = receiver.channel_lock.lock().unwrap();

            loop {
                match receiver.land_patches.try_recv() {
                    Ok(patches) => {
                        for patch in patches {
                            handler(patch);
                        }
                    }
                    Err(crossbeam_channel::TryRecvError::Empty) => {
                        break;
                    }
                    Err(crossbeam_channel::TryRecvError::Disconnected) => {
                        disconnected.push(region_id.clone());
                        break;
                    }
                }
            }

            drop(lock);
        }

        // Remove disconnected receivers.
        for region_id in disconnected.iter() {
            self.receivers.remove(region_id);
        }
    }
}

/// Contains the receivers for one region.
struct Receiver {
    channel_lock: Arc<Mutex<()>>,
    land_patches: crossbeam_channel::Receiver<Vec<Patch>>,
}

pub struct TerrainService {
    receiver: Cell<Option<Receiver>>,
}

impl Service for TerrainService {
    fn register_service(handlers: &mut message_handlers::Handlers, log: &Log) -> Self {
        // TODO: Refactor this code a bit, especially the many cloned Arc are a bit
        // ugly.
        let (patch_tx, patch_rx) = crossbeam_channel::bounded(100);
        let patch_tx = Arc::new(Mutex::new(patch_tx));
        let channel_lock = Arc::new(Mutex::new(()));
        let channel_lock2 = Arc::clone(&channel_lock);
        let logger = Arc::new(Logger::root(log.clone(), o!("service" => "TerrainService")));

        let handler = move |msg: MessageInstance, context: &message_handlers::HandlerContext| {
            let patch_tx = Arc::clone(&patch_tx);
            let channel_lock3 = Arc::clone(&channel_lock2);
            match msg {
                MessageInstance::LayerData(msg) => {
                    debug!(logger, "Received new layer data msg");
                    let logger2 = Arc::clone(&logger);
                    let logger3 = Arc::clone(&logger);
                    let patches = context.cpupool.spawn_fn(move || {
                        extract_land_patch(&msg).map(|patches| {
                            debug!(logger2, "Decoding layer data ok.");
                            // Wait for potential reads of the values.
                            let lock = channel_lock3.lock().unwrap();
                            let tx = patch_tx.lock().unwrap();
                            tx.send(patches).unwrap();
                            drop(lock);
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

        let receiver = Receiver {
            channel_lock,
            land_patches: patch_rx,
        };

        TerrainService {
            receiver: Cell::new(Some(receiver)),
        }
    }
}
