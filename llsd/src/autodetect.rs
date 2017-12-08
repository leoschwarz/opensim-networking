//! Handles auto detecting the representation of the format.

use data::Value;

lazy_static! {
    pub static ref PREFIX_BINARY: Vec<u8> = "<? LLSD/BINARY ?>\n".bytes().collect();
    pub static ref PREFIX_XML: Vec<u8> = "<?xml ".bytes().collect();
    pub static ref PREFIX_NOTATION: Vec<u8> = "<?llsd/notation?>\n".bytes().collect();
}

#[derive(Debug)]
pub enum Representation {
    Binary,
    Xml,
    Notation,
    Unknown,
}

fn determine_representation(buf: &[u8]) -> Representation {
    if buf.starts_with(&PREFIX_BINARY[..]) {
        Representation::Binary
    } else if buf.starts_with(&PREFIX_XML[..]) {
        Representation::Xml
    } else if buf.starts_with(&PREFIX_NOTATION[..]) {
        Representation::Notation
    } else {
        Representation::Unknown
    }
}

#[derive(Debug, ErrorChain)]
#[error_chain(error = "ReadError")]
#[error_chain(result = "")]
pub enum ReadErrorKind {
    #[error_chain(link = "::binary::ReadError")] ReadBinary(::binary::ReadErrorKind),

    #[error_chain(link = "::xml::ReadError")] ReadXml(::xml::ReadErrorKind),

    #[error_chain(custom)] InvalidRepresentation(Representation),
}

/// Read a value from either Binary or XML LLSD representation.
///
/// The format will be detected automatically by checking the document header.
pub fn read_value(buf: &[u8]) -> Result<Value, ReadError> {
    let repr = determine_representation(buf);
    match repr {
        Representation::Binary => {
            use std::io::BufReader;

            let mut reader = BufReader::new(&buf[PREFIX_BINARY.len()..]);
            Ok(::binary::read_value(&mut reader)?)
        }
        Representation::Xml => Ok(::xml::read_value(buf)?),
        Representation::Notation | Representation::Unknown => {
            Err(ReadErrorKind::InvalidRepresentation(repr).into())
        }
    }
}
