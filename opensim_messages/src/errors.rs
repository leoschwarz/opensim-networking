use UuidParseError;

#[derive(Debug, ErrorChain)]
#[error_chain(error = "ReadError")]
#[error_chain(result = "ReadResult")]
pub enum ReadErrorKind {
    #[error_chain(foreign)]
    IoError(::std::io::Error),

    #[error_chain(foreign)]
    ParseUuid(UuidParseError),

    /// No message struct for the message to be read was found.
    #[error_chain(custom)]
    #[error_chain(description = r#"|_| "No message struct for the message to be read was found.""#)]
    #[error_chain(display = r#"|_| write!(f, "No message struct for the message to be read was found.")"#)]
    UnknownMessageNumber(u32),
}
