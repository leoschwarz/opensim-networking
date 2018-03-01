use circuit::{message_handlers, MessageSender};

pub trait Service {
    fn register_service(
        handlers: &mut message_handlers::Handlers,
        message_sender: MessageSender,
    ) -> Self;
}

pub mod region_handle;
