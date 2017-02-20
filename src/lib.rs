#[macro_use]
extern crate bitflags;
extern crate byteorder;
extern crate crypto;
extern crate futures;
extern crate hyper;
extern crate mio;
extern crate nalgebra;
extern crate regex;
extern crate time;
extern crate tokio_core;
extern crate tokio_proto;
extern crate ttl_cache;
extern crate url;
extern crate uuid;
extern crate xmlrpc;

// Type definitions.
pub use nalgebra::{Vector3, Vector4, Quaternion, UnitQuaternion};
pub use std::net::Ipv4Addr as Ip4Addr;
pub use std::net::IpAddr;

pub type IpPort = u16;
pub use uuid::Uuid;

mod util;
pub mod messages;
pub mod login;
pub mod packet;
pub mod circuit;

