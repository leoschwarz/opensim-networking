use UuidParseError;
use std::io::Error as IoError;

// TODO: Evaluate if we need context here.
// - The original version with error-chain and backtrace, resulted in a 35 MiB
// debug build. - Full backtraces could quickly blow this up.
// - However for meaningful debugging either here or in the logger, some
// information about the message should be stored. (Ideally the binary
// version of the message failing to   decode should be available.)

#[derive(Debug, Fail)]
pub enum ReadError {
    #[fail(display = "IO error: {}", _0)]
    IoError(#[cause] ::std::io::Error),

    #[fail(display = "Parse UUID: {}", _0)]
    ParseUuid(UuidParseError),

    /*
    #[fail(display = "Failed parsing value: {}", _0)]
    ParseValue(ParseError),
    */
    #[fail(display = "No definition for message number {} found.", _0)]
    UnknownMessageNumber(u32),
}

impl From<UuidParseError> for ReadError {
    fn from(e: UuidParseError) -> Self {
        ReadError::ParseUuid(e)
    }
}

impl From<IoError> for ReadError {
    fn from(e: IoError) -> Self {
        ReadError::IoError(e)
    }
}
