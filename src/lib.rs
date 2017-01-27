#[macro_use]
extern crate bitflags;
extern crate byteorder;
extern crate crypto;
extern crate hyper;
extern crate mio;
extern crate nalgebra;
extern crate regex;
extern crate time;
extern crate tokio_core;
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


// TODO move to right place later.
use tokio_core::net::{UdpSocket, UdpCodec};

pub struct OpensimCodec;

/*
impl UdpCodec for OpensimCodec {
    type In = circuit::Packet;
    type Out = circuit::Packet;

    fn decode(&mut self, src: &SocketAddr, buf: &[u8]) -> Result<Self::In> {
        // TODO parse the packet using Packet::read_from
    }

    fn encode(&mut self, msg: Self::Out, buf: &mut Vec<u8>) -> SocketAddr {

    }
}
*/
