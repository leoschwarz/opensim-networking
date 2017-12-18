use std::collections::HashMap;
use std::sync::Arc;
use simulator::{Simulator, SimLocator, ConnectError, ConnectInfo};
use tokio_core::reactor::Handle;
use logging::Log;

#[derive(Debug)]
pub enum SimManagerError {
    NotFound,
    Connect(ConnectError),
}

impl From<ConnectError> for SimManagerError {
    fn from(e: ConnectError) -> Self {
        SimManagerError::Connect(e)
    }
}

/// Manages the connections to multiple sims,
/// and is responsible for executing the main event loop.
pub struct SimManager {
    /// How many connections to allow at most at once.
    connection_limit: u16,

    log: Log,
    handle: Handle,
    connections: HashMap<SimLocator, Arc<Simulator>>,
}

impl SimManager {
    pub fn new(handle: Handle, log: Log) -> Self {
        SimManager {
            connection_limit: 100,
            log: log,
            handle: handle,
            connections: HashMap::new(),
        }
    }

    fn connect_sim(&self, connect_info: &ConnectInfo) -> Result<Simulator, ConnectError> {
        let handlers = HashMap::new();
        Simulator::connect(connect_info, handlers, self.handle.clone(), &self.log)
    }

    pub fn get_sim(&mut self,
                   locator: &SimLocator,
                   connect_info: Option<&ConnectInfo>) -> Result<Arc<Simulator>, SimManagerError> {
        if let Some(sim) = self.connections.get(locator) {
            return Ok(Arc::clone(sim));
        }
        if let Some(info) = connect_info {
            let sim = Arc::new(self.connect_sim(info)?);
            let locator = sim.locator();
            self.connections.insert(locator, Arc::clone(&sim));
            Ok(sim)
        } else {
            Err(SimManagerError::NotFound)
        }
    }
}
