use capabilities::Capabilities;
use circuit::{message_handlers, MessageSender};
use logging::Log;
use std::cell::RefCell;
use std::sync::Arc;
use types::Uuid;

pub trait Service {
    fn register_service(
        handlers: &mut message_handlers::Handlers,
        circuit_data: CircuitDataHandle,
        log: &Log,
    ) -> Self;
}

/// Provides acess to the CircuitData once it is available.
///
/// The reason it has to be passed like this is that it will not be available
/// when registering the services. It is guaranteed that the value will be
/// available when handlers are called.
#[derive(Clone)]
pub struct CircuitDataHandle(Arc<RefCell<Option<Arc<CircuitData>>>>);

impl CircuitDataHandle {
    pub(crate) fn new() -> Self {
        CircuitDataHandle(Arc::new(RefCell::new(None)))
    }

    pub(crate) fn set(&self, data: CircuitData) {
        self.0.replace(Some(Arc::new(data)));
    }

    /// Return the available CircuitData.
    ///
    /// # Panics
    ///
    /// If data is not yet available. This should not happen outside the
    /// register_service methods.
    pub fn unwrap(&self) -> Arc<CircuitData> {
        let option = self.0.borrow();
        Arc::clone(option.as_ref().unwrap())
    }
}

pub struct CircuitData {
    pub capabilities: Capabilities,
    pub region_id: Uuid,
    pub message_sender: MessageSender,
}

pub mod region_handle;
pub mod terrain;
