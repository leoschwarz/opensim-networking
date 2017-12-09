//! Contains some modules of functionality which are commonly needed in client
//! applications, allowing the API user to depend upon these helpers instead of
//! having to deal with the corresponding messages manually.

pub mod agent_update;

/*
// TODO: Consider whether for our purposes we want to keep this composable, or just
//       implement the things we need and avoid the boxing?

use messages::MessageInstance;

/// Trait to enable composable handling of messages received through the circuit.
pub trait System {
    /// Called as soon as the connection is established.
    ///
    /// This allows a System instance to be passed to a Circuit instance,
    /// before starting the communication, so that no message will be missed.
    fn handle_start(&self);

    /// Called after connection is lost, or immediately before breaking the connection.
    fn handle_stop(&self);

    /// Called when a message is received, and it was not yet eaten by any
    /// of the System instances with a higher priority.
    fn handle_message(&self, message: MessageInstance);
}

struct SystemInstance {
    instance: Box<System>,
    priority: f32,
}
*/
