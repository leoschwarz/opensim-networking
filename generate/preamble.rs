use {Vector3, Vector4, Quaternion, UnitQuaternion, Ip4Addr, IpPort, Uuid};
use std::io::{Read, Write};
use byteorder::{LittleEndian, BigEndian, ReadBytesExt, WriteBytesExt};

pub enum WriteMessageError {
    IoError(::std::io::Error)
}

pub enum ReadMessageError {
    IoError(::std::io::Error),

    /// There was an issue parsing one of the types.
    ParseError,

    /// No message struct for the message to be read was found.
    UnknownMessageNumber
}

impl From<::std::io::Error> for WriteMessageError {
    fn from(e: ::std::io::Error) -> WriteMessageError {
        WriteMessageError::IoError(e)
    }
}

impl From<::std::io::Error> for ReadMessageError {
    fn from(e: ::std::io::Error) -> ReadMessageError {
        ReadMessageError::IoError(e)
    }
}

impl From<::uuid::ParseError> for ReadMessageError {
    fn from(e: ::uuid::ParseError) -> ReadMessageError {
        ReadMessageError::ParseError
    }
}

pub type WriteMessageResult = Result<(), WriteMessageError>;

pub trait Message {
    /// Write the message to a buffer for network transmission.
    fn write_to<W: ?Sized>(&self, buffer: &mut W) -> WriteMessageResult where W: Write;

    /// Read the message from a buffer obtained from the network.
    /// When this function is invoked it is assumed that the message number has
    /// already been read from the buffer object and the body of the message
    /// is at the initial buffer position.
    fn read_from<R: ?Sized>(buffer: &mut R) -> Result<MessageInstance, ReadMessageError> where R: Read;
}

