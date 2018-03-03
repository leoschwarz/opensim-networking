use circuit::{message_handlers, MessageSender};
use logging::Log;

pub trait Service {
    fn register_service(handlers: &mut message_handlers::Handlers, log: &Log) -> Self;

    /// Called by the initialization code once the message sender becomes
    /// available.
    // TODO: This is a rather ugly solution to the problem, ideally this could be
    // passed with the register_service function, however note the
    // complications in the usage in the Simulator initalization.
    fn register_message_sender(&mut self, _sender: MessageSender) {}
}

pub mod region_handle;
pub mod terrain;
