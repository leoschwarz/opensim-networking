use tokio_core::reactor::Core;
use std::collections::HashMap;
use simulator::{Simulator, SimLocator, ConnectError};
use logging::Log;

pub enum SimManagerError {

}

/// Manages the connections to multiple sims,
/// and is responsible for executing the main event loop.
pub struct SimManager {
    /// How many connections to allow at most at once.
    connection_limit: u16,

    log: Log,
    core: Core,
    connections: HashMap<SimLocator, Simulator>,
}

impl SimManager {
    fn connect_sim(&self) -> Result<Simulator, ConnectError> {

    }

    pub fn get_sim(&self, locator: &SimLocator) -> Result<Simulator, SimManagerError> {

    }
}
