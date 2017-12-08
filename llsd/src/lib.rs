extern crate byteorder;
extern crate chrono;
extern crate data_encoding;
#[macro_use]
extern crate derive_error_chain;
extern crate error_chain;
#[macro_use]
extern crate lazy_static;
extern crate quick_xml;
extern crate regex;
extern crate uuid;
extern crate xml as xml_crate;

pub mod data;
pub mod binary;
pub mod xml;

mod autodetect;

pub use autodetect::{read_value, PREFIX_BINARY};
