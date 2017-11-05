// TODO: Remove at some later time.
#![allow(dead_code)]

#[macro_use]
extern crate bitflags;
extern crate bitreader;
extern crate byteorder;
extern crate crypto;
#[macro_use]
extern crate derive_error_chain;
extern crate error_chain;
extern crate futures;
extern crate mio;
extern crate regex;
extern crate reqwest;
extern crate ttl_cache;
extern crate xmlrpc;

pub extern crate opensim_messages as messages;
extern crate opensim_types as types;
pub use types::*;

mod util;
pub mod circuit;
pub mod layer_data;
pub mod logging;
pub mod login;
pub mod packet;
pub mod systems;
