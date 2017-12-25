// TODO: Remove at some later time.
#![allow(dead_code)]
#![feature(proc_macro, conservative_impl_trait, generators)]

#[macro_use]
extern crate bitflags;
extern crate bitreader;
extern crate byteorder;
extern crate crypto;
#[macro_use]
extern crate derive_error_chain;
extern crate error_chain;
#[macro_use]
extern crate failure;
extern crate futures_await as futures;
extern crate hyper;
extern crate image;
extern crate jpeg2000;
#[macro_use]
extern crate lazy_static;
extern crate llsd;
extern crate mio;
extern crate nalgebra;
extern crate regex;
extern crate reqwest;
extern crate rmp_serde;
extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate slog;
extern crate slog_async;
extern crate slog_term;
extern crate tokio_core;
extern crate xmlrpc;

pub extern crate opensim_messages as messages;
pub extern crate opensim_types as types;

mod util;
pub mod capabilities;
pub mod circuit;
pub mod data;
pub mod layer_data;
pub mod logging;
pub mod login;
pub mod packet;
pub mod simulator;
pub mod systems;
pub mod textures;

/// experimental
pub mod object_update;
