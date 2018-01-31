extern crate byteorder;
extern crate chrono;
extern crate data_encoding;
#[macro_use]
extern crate failure;
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
