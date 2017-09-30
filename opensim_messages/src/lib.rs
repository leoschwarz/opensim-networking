extern crate byteorder;
extern crate opensim_types;

use opensim_types::*;
use std::io::{Read, Write};

mod errors;
pub use self::errors::ReadMessageError;

pub type WriteMessageResult = ::std::io::Result<()>;

pub trait Message {
    /// Write the message to a buffer for network transmission.
    fn write_to<W: ?Sized>(&self, buffer: &mut W) -> WriteMessageResult where W: Write;

    /// Read the message from a buffer obtained from the network.
    /// When this function is invoked it is assumed that the message number has
    /// already been read from the buffer object and the body of the message
    /// is at the initial buffer position.
    fn read_from<R: ?Sized>(buffer: &mut R) -> Result<MessageInstance, ReadMessageError> where R: Read;
}

/// Contains all available messages.
pub mod all;

// TODO rethink this later
pub use all::*;
