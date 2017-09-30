use std::io::Error as IoError;
use std::io::ErrorKind as IoErrorKind;
use UuidParseError;

// TODO: this whole thing will need more work once i have to debug something :-)

pub enum ReadMessageError {
    IoError(IoError),

    /// There was an issue parsing one of the types.
    ParseError(ParseError),

    /// No message struct for the message to be read was found.
    UnknownMessageNumber(u32)
}

pub enum ParseError {
    Uuid(UuidParseError)
}

impl From<IoError> for ReadMessageError {
    fn from(e: IoError) -> ReadMessageError {
        ReadMessageError::IoError(e)
    }
}

impl From<UuidParseError> for ReadMessageError {
    fn from(e: UuidParseError) -> ReadMessageError {
        ReadMessageError::ParseError(ParseError::Uuid(e))
    }
}

impl From<ReadMessageError> for IoError {
    fn from(e: ReadMessageError) -> Self {
        // TODO: Better error handling.
        IoError::new(IoErrorKind::InvalidData, "reading the message failed")
    }
}
