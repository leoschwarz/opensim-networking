extern crate nalgebra;
extern crate uuid;

pub use nalgebra::{Vector3, Vector4, Quaternion, UnitQuaternion};
pub use std::net::Ipv4Addr as Ip4Addr;
pub use std::net::IpAddr;
pub use std::time::{Instant, Duration};

pub type IpPort = u16;

pub use uuid::Uuid;
pub use uuid::ParseError as UuidParseError;

pub type SequenceNumber = u32;

