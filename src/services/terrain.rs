//! Terrain data management.

use circuit::message_handlers;
use futures::{Async, Poll, Stream};
use layer_data::{extract_land_patch, Patch};
use services::Service;
use std::sync::{mpsc, Arc, Mutex};
use messages::{MessageInstance, MessageType};

pub struct TerrainService {
    land_patches: mpsc::Receiver<Vec<Patch>>,
}

impl Service for TerrainService {
    fn register_service(handlers: &mut message_handlers::Handlers) -> Self {
        let (patch_tx, patch_rx) = mpsc::channel();
        let patch_tx = Arc::new(Mutex::new(patch_tx));

        let handler = move |msg: MessageInstance, context: &message_handlers::HandlerContext| {
            let patch_tx = Arc::clone(&patch_tx);
            match msg {
                MessageInstance::LayerData(msg) => {
                    let _ = context.cpupool.spawn_fn(move || {
                        extract_land_patch(&msg)
                            .map(|patches| {
                                let tx = patch_tx.lock().unwrap();
                                tx.send(patches).unwrap();
                            })
                            .map_err(|_| {
                                // TODO
                                ()
                            })
                    });
                    Ok(())
                }
                _ => Err(message_handlers::Error {
                    msg: msg,
                    kind: message_handlers::ErrorKind::WrongHandler,
                }),
            }
        };
        handlers.register_type(MessageType::LayerData, Box::new(handler));

        TerrainService {
            land_patches: patch_rx,
        }
    }
}

impl TerrainService {
    pub fn receive_land<'a>(&'a self) -> Receiver<'a> {
        Receiver { service: self }
    }
}

struct Receiver<'a> {
    service: &'a TerrainService,
}

impl<'a> Stream for Receiver<'a> {
    type Item = Vec<Patch>;
    type Error = ();

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        match self.service.land_patches.try_recv() {
            Ok(patches) => Ok(Async::Ready(Some(patches))),
            Err(mpsc::TryRecvError::Empty) => Ok(Async::NotReady),
            Err(mpsc::TryRecvError::Disconnected) => Ok(Async::Ready(None)),
        }
    }
}
