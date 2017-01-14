#[macro_use]
extern crate bitflags;
extern crate byteorder;
extern crate crypto;
extern crate hyper;
extern crate mio;
extern crate nalgebra;
extern crate regex;
extern crate url;
extern crate uuid;
extern crate xmlrpc;

// Type definitions.
pub use nalgebra::{Vector3, Vector4, Quaternion, UnitQuaternion};
pub use std::net::Ipv4Addr as Ip4Addr;
pub use std::net::IpAddr;

pub type IpPort = u16;
pub use uuid::Uuid;

pub mod messages;
//mod parser;
pub mod login;
mod circuit;

