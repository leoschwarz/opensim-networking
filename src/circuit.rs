use {Ip4Addr, IpPort, Uuid};

use messages::{Message, MessageInstance, UseCircuitCode, UseCircuitCode_CircuitCode,
               WriteMessageResult};
use login::LoginResponse;
use packet::{Packet, PacketFlags, PACKET_RELIABLE};

use tokio_core::reactor::Core;
use tokio_core::net::{UdpCodec, UdpSocket, UdpFramed};
use std::net::{SocketAddr, SocketAddrV4};
use std::collections::VecDeque;
use time::Timespec;

use futures::stream::Stream;
use futures::sink::Sink;
use futures::Future;

/// We only consider viewer <-> simulator circuits.
/// TODO: Once there is IPv6 support in the opensim server, implement support for both v4 and v6.
///       This was not done yet since we would have to abort the code if we got IPv6 addresses
///       anywhere and would make the API less realiable.
pub struct Circuit {
    core: Core,

    /// A `Stream` and `Sink` interface to the encapsulated UdpSocket.
    /// This will be used to transport data to and from the simulator.
    transport: UdpFramed<OpensimCodec>,

    /// Socket address (contains address + port) of simulator.
    sim_address: SocketAddr,

    /// Maybe A 24 bit number (TODO: change type here???) created at the connection of any circuit.
    /// This number is stored in the packet header and incremented whenever a packet is sent
    /// from one end of the circuit to the other.
    ///
    /// For each connection and each direction messages are numbered with unique numbers in a
    /// sequential fashion. This number is incremented after sending each message.
    sequence_number: u32,
}

#[derive(Debug)]
pub enum CircuitInitiationError {
    IO(::std::io::Error),
}

impl From<::std::io::Error> for CircuitInitiationError {
    fn from(err: ::std::io::Error) -> Self {
        CircuitInitiationError::IO(err)
    }
}

/// A future returned by Circuit.send_packet indicating the current status of
/// a packet.
// TODO: Figure out if needed.
//pub struct SendPacket {
//    sequence_number: u32
//}

pub enum SendPacketError {
    /// The packet was to be sent reliable but not acknowledged in time.
    TimedOut,

    /// Any unresolved IoError instance.
    IoError(error: IoError),
}

impl Circuit {
    pub fn initiate(login_res: LoginResponse)
                    -> Result<Circuit, CircuitInitiationError> {

        // Create the eventloop.
        let mut core = Core::new().unwrap();
        let handle = core.handle();

        // Create the framed socket.
        let sim_address = SocketAddr::V4(SocketAddrV4::new(login_res.sim_ip, login_res.sim_port));
        let socket = UdpSocket::bind(&sim_address, &handle)?;
        let codec = OpensimCodec::new(sim_address.clone());
        let udp_framed = socket.framed(codec);

        // Create the circuit instance.
        let circuit = Circuit {
            core: core,
            transport: udp_framed,
            sim_address: sim_address,
            sequence_number: 1,
        };

        // Use the circuit code.
        let msg = UseCircuitCode {
            circuit_code: UseCircuitCode_CircuitCode {
                code: login_res.circuit_code,
                session_id: login_res.session_id.clone(),
                id: login_res.agent_id.clone(),
            },
        };
        let mut packet = Packet::new(msg, 1);
        packet.enable_flags(PACKET_RELIABLE);
        circuit.send_packet(packet);

        // TODO: Wait for an ack.

        // Finished.
        Ok(circuit)
    }

    /// Send a packet to the simulator.
    ///
    /// If the packet's PACKET_RELIABLE flag is set it will be sent reliably
    /// and retried multiple times until acknowledged by the remote simulator.
    fn send_packet(&self, packet: Packet) -> SendPacket {
        
    }

    /*
    fn send_packet(&self, packet: Packet) {
        let mut buf = Vec::new();
        packet.write_to(&mut buf);
        self.udp.send(packet);
    }

    // TODO: Move this method to the right location.
    fn send_message<M: Into<MessageInstance>>(&self, msg: M) {
        let packet = Packet::new(msg, self.sequence_number);
        self.udp = self.send_packet(packet);
    }*/
}

struct OpensimCodec {
    sim_address: SocketAddr
}

impl OpensimCodec {
    fn new(sim: SocketAddr) -> OpensimCodec {
        OpensimCodec {
            sim_address: sim
        }
    }
}

impl UdpCodec for OpensimCodec {
    type In = Packet;
    type Out = Packet;

    fn decode(&mut self, src: &SocketAddr, buf: &[u8]) -> Result<Self::In, ::std::io::Error> {
        Packet::read(buf)
    }

    fn encode(&mut self, packet: Self::Out, buf: &mut Vec<u8>) -> SocketAddr {
        packet.write_to(buf);
        self.sim_address
    }
}
